#![allow(dead_code)]

use core::arch::{asm, global_asm};
use core::sync::atomic::{AtomicU64, Ordering};

use crate::interrupt_vectors_v20::APIC_TIMER_VECTOR;
use crate::local_apic_v20::{
    X2APIC_LVT_TIMER_MSR, X2APIC_SIVR_MSR, X2APIC_TIMER_DIVIDE_MSR, X2APIC_TIMER_INITIAL_COUNT_MSR,
    cpuid_capabilities, enable_x2apic, read_apic_base, x2apic_eoi, x2apic_write,
};

pub const APIC_SPURIOUS_VECTOR: u8 = 0xFF;
pub const APIC_TIMER_DEFAULT_INITIAL_COUNT: u32 = 10_000_000;
pub const APIC_TIMER_DIVIDE_BY_16: u32 = 0b0011;

static APIC_TIMER_TICKS: AtomicU64 = AtomicU64::new(0);

global_asm!(
    r#"
    .global aw_apic_timer_isr_v25
    .type aw_apic_timer_isr_v25,@function

aw_apic_timer_isr_v25:
    push rax
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15

    cld

    # Preserve the exact interrupt-frame stack position in RBX.
    mov rbx, rsp

    # System V x86_64 requires RSP 16-byte aligned before CALL.
    and rsp, -16
    call aw_apic_timer_rust_v25
    mov rsp, rbx

    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rax
    iretq

    .size aw_apic_timer_isr_v25, .-aw_apic_timer_isr_v25
"#
);

unsafe extern "C" {
    fn aw_apic_timer_isr_v25();
}

#[unsafe(no_mangle)]
extern "C" fn aw_apic_timer_rust_v25() {
    APIC_TIMER_TICKS.fetch_add(1, Ordering::Relaxed);
    unsafe {
        x2apic_eoi();
    }
}

pub fn timer_ticks() -> u64 {
    APIC_TIMER_TICKS.load(Ordering::Acquire)
}

pub fn timer_fired() -> bool {
    timer_ticks() != 0
}

/// Installs the APIC timer vector into the kernel's IDT.
///
/// The gate is encoded and written by [`crate::interrupts::install_interrupt_gate`],
/// so the timer vector uses exactly the same audited descriptor layout, code
/// selector and privilege level as the CPU exception gates. The #DF/IST gate
/// and every other vector are left untouched.
///
/// # Safety
/// Must run at CPL0 after the kernel has installed its IDT with `lidt`.
/// Interrupts must be disabled while the descriptor is replaced.
pub unsafe fn install_timer_gate() -> Result<(), &'static str> {
    let handler = aw_apic_timer_isr_v25 as *const () as usize as u64;
    // The timer runs on the current kernel stack, so no IST switch (index 0).
    unsafe { crate::interrupts::install_interrupt_gate(APIC_TIMER_VECTOR, handler, 0) }
        .map_err(|_| "failed to encode APIC timer IDT gate")
}

/// Enables the x2APIC software-visible path needed by the one-shot test.
///
/// It intentionally does not disable the legacy PIC here. That policy belongs
/// to the platform IRQ transition layer.
///
/// # Safety
/// CPL0 only. Call during controlled BSP bring-up with interrupts disabled.
pub unsafe fn prepare_x2apic_for_timer() -> Result<(), &'static str> {
    let caps = cpuid_capabilities();
    if !caps.local_apic {
        return Err("CPUID reports no Local APIC");
    }
    if !caps.x2apic {
        return Err("CPUID reports no x2APIC");
    }

    let base = unsafe { enable_x2apic()? };
    if !base.enabled || !base.x2apic_enabled {
        return Err("x2APIC enable verification failed");
    }

    // APIC software enable bit (bit 8) + spurious vector.
    unsafe {
        x2apic_write(X2APIC_SIVR_MSR, (1u32 << 8) | APIC_SPURIOUS_VECTOR as u32);
    }

    Ok(())
}

/// Programs a one-shot Local APIC timer on vector 0x40.
///
/// # Safety
/// Vector 0x40 must already be installed in the loaded IDT.
/// x2APIC mode must already be enabled.
pub unsafe fn program_one_shot(initial_count: u32) -> Result<(), &'static str> {
    if initial_count == 0 {
        return Err("initial_count must be non-zero");
    }

    let base = unsafe { read_apic_base() };
    if !base.enabled || !base.x2apic_enabled {
        return Err("x2APIC is not enabled");
    }

    unsafe {
        x2apic_write(X2APIC_TIMER_DIVIDE_MSR, APIC_TIMER_DIVIDE_BY_16);
        // one-shot mode = timer mode bits 18:17 are zero.
        x2apic_write(X2APIC_LVT_TIMER_MSR, APIC_TIMER_VECTOR as u32);
        x2apic_write(X2APIC_TIMER_INITIAL_COUNT_MSR, initial_count);
    }

    Ok(())
}

/// Complete one-shot setup, but does NOT execute STI.
/// The caller keeps control over global interrupt-enable ordering.
///
/// # Safety
/// CPL0 only; must execute after the project's normal IDT/TSS/IST setup.
pub unsafe fn arm_one_shot_after_idt() -> Result<(), &'static str> {
    unsafe {
        asm!("cli", options(nomem, nostack, preserves_flags));
        install_timer_gate()?;
        prepare_x2apic_for_timer()?;
        program_one_shot(APIC_TIMER_DEFAULT_INITIAL_COUNT)?;
    }
    Ok(())
}

/// Enable maskable interrupts after arm_one_shot_after_idt().
///
/// # Safety
/// The caller must ensure the rest of the kernel is ready for maskable IRQs.
pub unsafe fn enable_interrupts_for_timer_test() {
    unsafe {
        asm!("sti", options(nomem, nostack, preserves_flags));
    }
}
