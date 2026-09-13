//! Cooperative round-robin scheduler with real context switching (dossier
//! section 8, roadmap P0 step 4).
//!
//! Ring 3 and syscalls proved the privilege boundary; this proves the other
//! half of step 4 - more than one thread of execution, switched by the kernel.
//! Several kernel threads each run on their own stack and hand control on with
//! `yield_now`; the switch saves the current thread's callee-saved registers and
//! stack pointer and restores the next thread's, so each resumes exactly where
//! it left off. It is cooperative for now (threads yield); wiring the same
//! switch into the timer interrupt makes it preemptive, which is the next step.
//!
//! It runs on the bootstrap processor with interrupts masked, so the shared
//! state needs no lock; the counters are atomics only so the assembly switch and
//! the Rust readers agree on memory.

use core::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

use crate::{debug_write, debug_write_u64};

const MAX_THREADS: usize = 3;
const STACK_SIZE: usize = 16 * 1024;
/// Total yields after which the running thread returns control to the kernel.
const TARGET_YIELDS: u32 = 30;

#[repr(C, align(16))]
struct Stack([u8; STACK_SIZE]);

static mut STACKS: [Stack; MAX_THREADS] = [const { Stack([0; STACK_SIZE]) }; MAX_THREADS];

/// Saved stack pointer per thread (updated by the assembly switch), the kernel's
/// own saved pointer, and the round-robin bookkeeping.
static THREAD_RSP: [AtomicU64; MAX_THREADS] = [const { AtomicU64::new(0) }; MAX_THREADS];
static MAIN_RSP: AtomicU64 = AtomicU64::new(0);
static CURRENT: AtomicUsize = AtomicUsize::new(0);
static NUM_THREADS: AtomicUsize = AtomicUsize::new(0);
static COUNTS: [AtomicU32; MAX_THREADS] = [const { AtomicU32::new(0) }; MAX_THREADS];
static TOTAL_YIELDS: AtomicU32 = AtomicU32::new(0);
static SWITCHES: AtomicU32 = AtomicU32::new(0);

core::arch::global_asm!(
    ".global aw_context_switch",
    ".type aw_context_switch,@function",
    // fn aw_context_switch(save_rsp = rdi, load_rsp = rsi)
    "aw_context_switch:",
    "    push rbp",
    "    push rbx",
    "    push r12",
    "    push r13",
    "    push r14",
    "    push r15",
    "    mov [rdi], rsp", // save the outgoing thread's stack pointer
    "    mov rsp, rsi",   // load the incoming thread's stack pointer
    "    pop r15",
    "    pop r14",
    "    pop r13",
    "    pop r12",
    "    pop rbx",
    "    pop rbp",
    "    ret",
    ".size aw_context_switch, .-aw_context_switch",
);

unsafe extern "C" {
    fn aw_context_switch(save_rsp: *mut u64, load_rsp: u64);
}

/// Lay out a thread's initial stack so the first switch into it "returns" to
/// `entry`, with zeroed callee-saved registers.
fn init_thread(index: usize, entry: extern "C" fn() -> !) {
    // SAFETY: `index < MAX_THREADS`; this addresses that thread's own stack.
    let top = unsafe {
        let base = core::ptr::addr_of_mut!(STACKS).cast::<Stack>().add(index) as u64;
        (base + STACK_SIZE as u64) & !0xf_u64
    };
    // Seven qwords: r15,r14,r13,r12,rbx,rbp (popped by the switch) then the
    // return address the final `ret` jumps to.
    let sp = top - 7 * 8;
    let slots = sp as *mut u64;
    // SAFETY: [sp, top) is inside this thread's stack; write the seven slots.
    unsafe {
        for offset in 0..6 {
            slots.add(offset).write(0);
        }
        slots.add(6).write(entry as usize as u64);
    }
    THREAD_RSP[index].store(sp, Ordering::Relaxed);
}

/// Hand control to the next thread in round-robin order.
fn yield_now() {
    let current = CURRENT.load(Ordering::Relaxed);
    let count = NUM_THREADS.load(Ordering::Relaxed);
    let next = (current + 1) % count;
    CURRENT.store(next, Ordering::Relaxed);
    SWITCHES.fetch_add(1, Ordering::Relaxed);
    // SAFETY: both stack pointers belong to live threads set up by init_thread;
    // the switch saves this thread's context and restores the next thread's.
    unsafe { aw_context_switch(THREAD_RSP[current].as_ptr(), THREAD_RSP[next].load(Ordering::Relaxed)) };
}

/// Return control to the kernel that called [`run`].
fn exit_to_main() -> ! {
    let current = CURRENT.load(Ordering::Relaxed);
    // SAFETY: save this (now finished) thread's pointer and load the kernel's.
    unsafe { aw_context_switch(THREAD_RSP[current].as_ptr(), MAIN_RSP.load(Ordering::Relaxed)) };
    // The kernel never switches back to this thread.
    loop {
        core::hint::spin_loop();
    }
}

/// Every thread's body: advance its own counter, yield, and once the whole run
/// has done enough yields, hand control back to the kernel.
fn thread_body(id: usize) -> ! {
    loop {
        COUNTS[id].fetch_add(1, Ordering::Relaxed);
        let total = TOTAL_YIELDS.fetch_add(1, Ordering::Relaxed) + 1;
        if total >= TARGET_YIELDS {
            exit_to_main();
        }
        yield_now();
    }
}

extern "C" fn thread0() -> ! {
    thread_body(0)
}
extern "C" fn thread1() -> ! {
    thread_body(1)
}
extern "C" fn thread2() -> ! {
    thread_body(2)
}

/// Start the threads and run until they hand control back.
fn run() {
    init_thread(0, thread0);
    init_thread(1, thread1);
    init_thread(2, thread2);
    NUM_THREADS.store(3, Ordering::Relaxed);
    CURRENT.store(0, Ordering::Relaxed);
    // SAFETY: save the kernel's context into MAIN_RSP and switch to thread 0.
    unsafe { aw_context_switch(MAIN_RSP.as_ptr(), THREAD_RSP[0].load(Ordering::Relaxed)) };
}

/// Prove cooperative multitasking: three threads interleave deterministically.
pub fn prove() {
    debug_write("AW_SCHED_BEGIN\n");
    run();

    // Control is back in the kernel. With three threads and 30 total yields,
    // round-robin gives each thread exactly ten turns.
    let mut all_ran = true;
    for (id, count_slot) in COUNTS.iter().enumerate() {
        let count = count_slot.load(Ordering::Relaxed);
        debug_write("AW_SCHED_THREAD id=");
        debug_write_u64(id as u64);
        debug_write(" count=");
        debug_write_u64(u64::from(count));
        debug_write("\n");
        if count != TARGET_YIELDS / 3 {
            all_ran = false;
        }
    }

    let total = TOTAL_YIELDS.load(Ordering::Relaxed);
    let switches = SWITCHES.load(Ordering::Relaxed);
    if all_ran && total == TARGET_YIELDS {
        debug_write("AW_SCHED_PROOF_OK threads=3 switches=");
        debug_write_u64(u64::from(switches));
        debug_write("\n");
    } else {
        debug_write("AW_SCHED_FAIL total=");
        debug_write_u64(u64::from(total));
        debug_write("\n");
    }
}
