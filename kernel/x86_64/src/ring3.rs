//! First transition to Ring 3 and back through a syscall (dossier section 9,
//! roadmap P0 step 4).
//!
//! This is the smallest honest proof that the kernel can drop to user privilege
//! and be re-entered under control:
//!
//! 1. a tiny user routine is written into a fresh frame and mapped
//!    user-accessible, executable, read-only, above the identity window;
//! 2. the SYSCALL MSRs are programmed (kernel entry in `LSTAR`, selector bases
//!    in `STAR`, `EFER.SCE` set);
//! 3. the kernel builds an `iretq` frame with the Ring 3 selectors and drops to
//!    CPL 3 at the user routine;
//! 4. the routine executes `syscall` with a known number and argument; the
//!    entry stub switches to the kernel stack, records what it received, and
//!    returns into the kernel rather than back to user - this is a one-shot
//!    proof, not a scheduler.
//!
//! Interrupts are masked throughout (the whole bootstrap runs with `IF` clear),
//! so no asynchronous entry from CPL 3 happens here and `GS` is never touched by
//! user code. `swapgs` and a real user `GS` base belong with the scheduler; the
//! entry stub here does not trust or swap `GS`, it only records and returns.

use core::sync::atomic::{AtomicU64, Ordering};

use aw_x86_paging::PageTableFlags;

use crate::interrupts::{USER_CODE_SELECTOR_RAW, USER_DATA_SELECTOR_RAW};
use crate::local_apic::{rdmsr, wrmsr};
use crate::{debug_write, debug_write_hex_u64, frame_allocator, interrupts, page_mapper};

const IA32_EFER: u32 = 0xc000_0080;
const IA32_STAR: u32 = 0xc000_0081;
const IA32_LSTAR: u32 = 0xc000_0082;
const IA32_FMASK: u32 = 0xc000_0084;
/// `EFER.SCE`: enables `syscall`/`sysret`.
const EFER_SYSCALL_ENABLE: u64 = 1 << 0;

/// First page of the Ring 3 window, well above the 4 GiB identity map so the
/// runtime mapper only meets interior tables on the way to it.
const USER_CODE_VA: u64 = 0x2_0000_0000;
const USER_STACK_VA: u64 = 0x2_0000_2000;
const USER_STACK_TOP: u64 = USER_STACK_VA + 0x1000;

/// The syscall the user routine makes, and the argument it carries. Both are
/// checked on return, so a stray or spurious entry cannot pass the proof.
const SYSCALL_NR: u64 = 0x101;
const SYSCALL_MAGIC: u64 = 0x5a11_c0de;

/// The user routine, hand-assembled and position independent:
/// `mov rax, SYSCALL_NR; mov rdi, SYSCALL_MAGIC; syscall; jmp .`
const USER_CODE: [u8; 18] = [
    0x48, 0xc7, 0xc0, 0x01, 0x01, 0x00, 0x00, // mov rax, 0x101
    0x48, 0xc7, 0xc7, 0xde, 0xc0, 0x11, 0x5a, // mov rdi, 0x5a11c0de
    0x0f, 0x05, // syscall
    0xeb, 0xfe, // jmp . (never reached: the handler returns to the kernel)
];

/// A dedicated Ring 0 stack for `TSS.rsp0`, used by any privilege change from
/// Ring 3 into the kernel (a fault taken while user code runs).
#[repr(C, align(16))]
struct KernelStack([u8; 16 * 1024]);

static mut RING0_STACK: KernelStack = KernelStack([0; 16 * 1024]);

fn ring0_stack_top() -> u64 {
    let base = core::ptr::addr_of!(RING0_STACK) as u64;
    (base + 16 * 1024) & !0xf_u64
}

// Shared with the assembly below. `SAVED_*` carry the kernel resume point across
// the excursion to Ring 3; `SYSCALL_SEEN_*` record what the syscall delivered.
#[used]
static SAVED_KERNEL_RSP: AtomicU64 = AtomicU64::new(0);
#[used]
static SAVED_KERNEL_RESUME: AtomicU64 = AtomicU64::new(0);
#[used]
static SYSCALL_SEEN_NR: AtomicU64 = AtomicU64::new(0);
#[used]
static SYSCALL_SEEN_ARG: AtomicU64 = AtomicU64::new(0);
#[used]
static SYSCALL_SEEN_RIP: AtomicU64 = AtomicU64::new(0);

const USER_DATA_SELECTOR: u64 = USER_DATA_SELECTOR_RAW as u64;
const USER_CODE_SELECTOR: u64 = USER_CODE_SELECTOR_RAW as u64;
/// The `RFLAGS` Ring 3 starts with: only the always-set reserved bit 1. `IF` is
/// clear, so no interrupt is taken while at CPL 3.
const USER_RFLAGS: u64 = 0x2;

core::arch::global_asm!(
    ".global aw_enter_ring3",
    ".global aw_syscall_entry",
    // fn aw_enter_ring3(user_rip = rdi, user_rsp = rsi)
    "aw_enter_ring3:",
    "    lea rax, [rip + aw_ring3_resume]",
    "    mov [rip + {saved_resume}], rax",
    "    mov [rip + {saved_rsp}], rsp",
    // iretq frame, high to low: SS, RSP, RFLAGS, CS, RIP.
    "    push {user_ss}",
    "    push rsi",
    "    push {user_flags}",
    "    push {user_cs}",
    "    push rdi",
    "    iretq",
    "aw_ring3_resume:",
    "    ret",
    // syscall entry: rax = number, rdi = argument, rcx = user RIP, r11 = user
    // RFLAGS, RSP still the user stack. No stack is used before the switch.
    "aw_syscall_entry:",
    "    mov [rip + {seen_nr}], rax",
    "    mov [rip + {seen_arg}], rdi",
    "    mov [rip + {seen_rip}], rcx",
    "    mov rsp, [rip + {saved_rsp}]",
    "    jmp [rip + {saved_resume}]",
    saved_resume = sym SAVED_KERNEL_RESUME,
    saved_rsp = sym SAVED_KERNEL_RSP,
    seen_nr = sym SYSCALL_SEEN_NR,
    seen_arg = sym SYSCALL_SEEN_ARG,
    seen_rip = sym SYSCALL_SEEN_RIP,
    user_ss = const USER_DATA_SELECTOR,
    user_cs = const USER_CODE_SELECTOR,
    user_flags = const USER_RFLAGS,
);

unsafe extern "C" {
    fn aw_enter_ring3(user_rip: u64, user_rsp: u64);
    fn aw_syscall_entry();
}

/// Program the SYSCALL MSRs so a `syscall` from Ring 3 lands in
/// [`aw_syscall_entry`] at CPL 0.
///
/// # Safety
/// CPL0. Must run before Ring 3 is entered.
unsafe fn enable_syscall() {
    // SAFETY: EFER already carries LME/LMA/NXE from bring-up; this only adds SCE.
    let efer = unsafe { rdmsr(IA32_EFER) };
    unsafe { wrmsr(IA32_EFER, efer | EFER_SYSCALL_ENABLE) };

    // STAR: syscall loads kernel CS from bits [47:32] (0x08, SS becomes 0x10);
    // sysret would derive the user selectors from bits [63:48] (0x20 -> SS 0x28,
    // CS 0x30). sysret is not used here, but the field is set correctly for it.
    let star = (0x0020_u64 << 48) | (0x0008_u64 << 32);
    unsafe { wrmsr(IA32_STAR, star) };
    unsafe { wrmsr(IA32_LSTAR, aw_syscall_entry as *const () as u64) };
    // Clear IF, DF, TF and AC on entry, so the handler runs with interrupts off
    // and a sane string direction regardless of the user's RFLAGS.
    let fmask = (1_u64 << 9) | (1 << 10) | (1 << 8) | (1 << 18);
    unsafe { wrmsr(IA32_FMASK, fmask) };
}

/// Enter Ring 3, run one syscall, and prove the round trip.
pub fn prove() {
    debug_write("AW_RING3_BEGIN\n");

    // User stack: writable, never executable, user-accessible.
    let Some(stack_frame) = frame_allocator::allocate() else {
        debug_write("AW_RING3_FAIL reason=no_stack_frame\n");
        return;
    };
    let stack_flags = PageTableFlags::USER_ACCESSIBLE
        .union(PageTableFlags::WRITABLE)
        .union(PageTableFlags::NO_EXECUTE);
    // SAFETY: CPL0; USER_STACK_VA is unused and the frame was just allocated.
    if let Err(error) = unsafe { page_mapper::map_page(USER_STACK_VA, stack_frame, stack_flags) } {
        debug_write("AW_RING3_FAIL reason=map_stack_");
        debug_write(error.name());
        debug_write("\n");
        return;
    }

    // User code: written through the frame's identity address, then mapped
    // user-accessible, executable and read-only.
    let Some(code_frame) = frame_allocator::allocate() else {
        debug_write("AW_RING3_FAIL reason=no_code_frame\n");
        return;
    };
    // SAFETY: the frame is in the identity window, writable there; we copy the
    // routine in before mapping it read-only-executable for user.
    unsafe {
        core::ptr::copy_nonoverlapping(USER_CODE.as_ptr(), code_frame as *mut u8, USER_CODE.len());
    }
    let code_flags = PageTableFlags::USER_ACCESSIBLE;
    // SAFETY: CPL0; USER_CODE_VA is unused and the frame holds the routine.
    if let Err(error) = unsafe { page_mapper::map_page(USER_CODE_VA, code_frame, code_flags) } {
        debug_write("AW_RING3_FAIL reason=map_code_");
        debug_write(error.name());
        debug_write("\n");
        return;
    }

    // A Ring 0 stack for any privilege change out of Ring 3, then the SYSCALL
    // MSRs.
    // SAFETY: CPL0, single core, before Ring 3 is entered.
    unsafe {
        interrupts::set_bootstrap_rsp0(ring0_stack_top());
        enable_syscall();
    }

    debug_write("AW_RING3_MAP_OK code_va=");
    debug_write_hex_u64(USER_CODE_VA);
    debug_write(" stack_va=");
    debug_write_hex_u64(USER_STACK_VA);
    debug_write("\n");

    // Drop to Ring 3 at the user routine. Returns here once the routine's
    // syscall has been serviced.
    // SAFETY: the user page is mapped executable at CPL3, the stack is mapped
    // writable, the SYSCALL MSRs and rsp0 are set, and interrupts are masked.
    unsafe { aw_enter_ring3(USER_CODE_VA, USER_STACK_TOP) };

    let nr = SYSCALL_SEEN_NR.load(Ordering::Acquire);
    let arg = SYSCALL_SEEN_ARG.load(Ordering::Relaxed);
    let rip = SYSCALL_SEEN_RIP.load(Ordering::Relaxed);
    debug_write("AW_SYSCALL_RECEIVED nr=");
    debug_write_hex_u64(nr);
    debug_write(" arg=");
    debug_write_hex_u64(arg);
    debug_write(" rip=");
    debug_write_hex_u64(rip);
    debug_write("\n");

    // The syscall must carry the number and argument the user routine set, and
    // its return address must land inside the user code page - proof the call
    // came from CPL 3 at the address we mapped, not from anywhere in the kernel.
    let rip_in_user_page = (USER_CODE_VA..USER_CODE_VA + 0x1000).contains(&rip);
    if nr == SYSCALL_NR && arg == SYSCALL_MAGIC && rip_in_user_page {
        debug_write("AW_RING3_PROOF_OK\n");
    } else {
        debug_write("AW_RING3_FAIL reason=bad_syscall\n");
    }
}
