//! Local APIC timer: real hardware interrupt delivery on the bootstrap CPU.
//!
//! This is the kernel's first genuine interrupt source, and the dossier holds
//! it to a delivery proof rather than a build proof (IRQ-002/IRQ-003): the ISR
//! must actually run, the tick counter must advance monotonically across
//! several deliveries without deadlocking, and masking the vector must stop it.
//! [`run_delivery_proof`] performs exactly that sequence, including the
//! negative test.

use core::arch::{asm, global_asm};
use core::sync::atomic::{AtomicU64, Ordering};

use crate::interrupt_vectors::APIC_TIMER_VECTOR;
use crate::local_apic::{
    x2apic_eoi, x2apic_read, x2apic_write, ApicBase, X2APIC_LVT_TIMER_MSR, X2APIC_SIVR_MSR,
    X2APIC_TIMER_DIVIDE_MSR, X2APIC_TIMER_INITIAL_COUNT_MSR,
};

pub const APIC_SPURIOUS_VECTOR: u8 = 0xff;

/// Divides the APIC bus clock by 16, so a moderate initial count still yields
/// a period long enough for the ISR to complete under TCG emulation.
pub const APIC_TIMER_DIVIDE_BY_16: u32 = 0b0011;

/// Periodic reload value. Short enough that the proof completes quickly under
/// QEMU/TCG, long enough that the ISR is never re-entered before it returns.
pub const APIC_TIMER_PERIODIC_COUNT: u32 = 1_000_000;

/// `LVT_TIMER` bit 16: masks the vector without disturbing the running count.
const LVT_MASKED: u32 = 1 << 16;
/// `LVT_TIMER` bits 18:17 = 01: periodic mode.
const LVT_PERIODIC: u32 = 1 << 17;

static APIC_TIMER_TICKS: AtomicU64 = AtomicU64::new(0);

global_asm!(
    r#"
    .global aw_apic_timer_isr
    .type aw_apic_timer_isr,@function

aw_apic_timer_isr:
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
    call aw_apic_timer_dispatch
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

    .size aw_apic_timer_isr, .-aw_apic_timer_isr
"#
);

unsafe extern "C" {
    fn aw_apic_timer_isr();
}

#[unsafe(no_mangle)]
extern "C" fn aw_apic_timer_dispatch() {
    APIC_TIMER_TICKS.fetch_add(1, Ordering::Relaxed);
    // SAFETY: CPL0 interrupt context on a CPU whose x2APIC was enabled before
    // the vector was unmasked. EOI must be signalled before `iretq` or the
    // Local APIC keeps this priority level blocked and no further timer
    // interrupt is delivered.
    unsafe { x2apic_eoi() };
}

#[must_use]
pub fn timer_ticks() -> u64 {
    APIC_TIMER_TICKS.load(Ordering::Acquire)
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
    let handler = aw_apic_timer_isr as *const () as usize as u64;
    // The timer runs on the current kernel stack, so no IST switch (index 0).
    unsafe { crate::interrupts::install_interrupt_gate(APIC_TIMER_VECTOR, handler, 0) }
        .map_err(|_| "failed to encode APIC timer IDT gate")
}

/// Enables x2APIC mode and the APIC software-enable bit.
///
/// It intentionally does not disable the legacy PIC here. That policy belongs
/// to the platform IRQ transition layer.
///
/// # Safety
/// CPL0 only. Call during controlled CPU bring-up with interrupts disabled.
pub unsafe fn prepare_x2apic() -> Result<ApicBase, &'static str> {
    let caps = crate::local_apic::cpuid_capabilities();
    if !caps.local_apic {
        return Err("CPUID reports no Local APIC");
    }
    if !caps.x2apic {
        return Err("CPUID reports no x2APIC");
    }

    let base = unsafe { crate::local_apic::enable_x2apic()? };
    if !base.enabled || !base.x2apic_enabled {
        return Err("x2APIC enable verification failed");
    }

    // APIC software enable bit (bit 8) + spurious vector.
    unsafe { x2apic_write(X2APIC_SIVR_MSR, (1 << 8) | u32::from(APIC_SPURIOUS_VECTOR)) };

    Ok(base)
}

/// Programs the Local APIC timer in periodic mode on [`APIC_TIMER_VECTOR`].
///
/// The vector is programmed **masked**, so arming it never races with the
/// caller's own decision about when to set `IF`.
///
/// # Safety
/// [`APIC_TIMER_VECTOR`] must already be installed in the loaded IDT and
/// x2APIC mode must already be enabled.
pub unsafe fn program_periodic(initial_count: u32) -> Result<(), &'static str> {
    if initial_count == 0 {
        return Err("initial_count must be non-zero");
    }

    let base = unsafe { crate::local_apic::read_apic_base() };
    if !base.enabled || !base.x2apic_enabled {
        return Err("x2APIC is not enabled");
    }

    unsafe {
        x2apic_write(X2APIC_TIMER_DIVIDE_MSR, APIC_TIMER_DIVIDE_BY_16);
        x2apic_write(
            X2APIC_LVT_TIMER_MSR,
            LVT_MASKED | LVT_PERIODIC | u32::from(APIC_TIMER_VECTOR),
        );
        x2apic_write(X2APIC_TIMER_INITIAL_COUNT_MSR, initial_count);
    }

    Ok(())
}

/// Sets or clears `LVT_TIMER.MASK`, leaving mode and vector untouched.
///
/// # Safety
/// CPL0 only, after [`program_periodic`].
pub unsafe fn set_timer_masked(masked: bool) {
    // SAFETY: the caller guarantees CPL0 after the LVT has been programmed.
    let current = unsafe { x2apic_read(X2APIC_LVT_TIMER_MSR) };
    let updated = if masked {
        current | LVT_MASKED
    } else {
        current & !LVT_MASKED
    };
    unsafe { x2apic_write(X2APIC_LVT_TIMER_MSR, updated) };
}

/// Complete periodic setup with the vector still masked. Does not execute
/// `sti`: the caller keeps control over global interrupt-enable ordering.
///
/// # Safety
/// CPL0 only; must execute after the kernel's normal IDT/TSS/IST setup.
pub unsafe fn arm_periodic_after_idt() -> Result<(), &'static str> {
    unsafe {
        asm!("cli", options(nomem, nostack, preserves_flags));
        install_timer_gate()?;
        prepare_x2apic()?;
        program_periodic(APIC_TIMER_PERIODIC_COUNT)?;
    }
    Ok(())
}

/// # Safety
/// The caller must ensure the rest of the kernel is ready for maskable IRQs.
pub unsafe fn enable_interrupts() {
    unsafe { asm!("sti", options(nomem, nostack, preserves_flags)) };
}

/// # Safety
/// CPL0 only.
pub unsafe fn disable_interrupts() {
    unsafe { asm!("cli", options(nomem, nostack, preserves_flags)) };
}

/// Bounded busy-wait budget for one phase of the delivery proof. Large enough
/// for several periods under QEMU/TCG, small enough that a dead timer fails the
/// proof in seconds rather than hanging the boot.
const PROOF_SPIN_BUDGET: u32 = 200_000_000;

/// Outcome of [`run_delivery_proof`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryProof {
    /// The ISR ran at least `required_ticks` times, stopped while masked and
    /// resumed once unmasked.
    Passed {
        ticks_after_run: u64,
        ticks_while_masked: u64,
        ticks_after_unmask: u64,
    },
    /// The vector was unmasked but the ISR never reached `required_ticks`.
    NotDelivered { ticks: u64 },
    /// The counter kept advancing while `LVT_TIMER.MASK` was set, so the
    /// increments are not attributable to real interrupt delivery.
    MaskIneffective { before: u64, after: u64 },
    /// Delivery did not resume after clearing the mask.
    DidNotResume { ticks: u64 },
}

fn spin_until(budget: u32, mut done: impl FnMut() -> bool) -> bool {
    let mut spins = 0;
    while spins < budget {
        if done() {
            return true;
        }
        spins += 1;
        core::hint::spin_loop();
    }
    done()
}

/// Prove real interrupt delivery, then prove the counter is driven by it.
///
/// A monotonically increasing counter on its own does not distinguish a real
/// ISR from a polling artefact, so the proof includes the dossier's required
/// negative test: with `LVT_TIMER.MASK` set the counter must be frozen, and it
/// must start moving again when the mask is cleared.
///
/// # Safety
/// CPL0 only, after [`arm_periodic_after_idt`]. Returns with interrupts
/// disabled and the timer masked, whatever the outcome.
pub unsafe fn run_delivery_proof(required_ticks: u64) -> DeliveryProof {
    unsafe {
        set_timer_masked(false);
        enable_interrupts();
    }

    let delivered = spin_until(PROOF_SPIN_BUDGET, || timer_ticks() >= required_ticks);
    let ticks_after_run = timer_ticks();
    if !delivered {
        unsafe {
            disable_interrupts();
            set_timer_masked(true);
        }
        return DeliveryProof::NotDelivered {
            ticks: ticks_after_run,
        };
    }

    // Negative test: mask the vector with IF still set. Real deliveries stop.
    unsafe { set_timer_masked(true) };
    let before = timer_ticks();
    // One in-flight interrupt may already have been accepted when the mask was
    // written, so settle first, then require a strictly frozen window.
    spin_until(PROOF_SPIN_BUDGET / 20, || false);
    let settled = timer_ticks();
    spin_until(PROOF_SPIN_BUDGET / 20, || false);
    let ticks_while_masked = timer_ticks();
    if ticks_while_masked != settled {
        unsafe {
            disable_interrupts();
        }
        return DeliveryProof::MaskIneffective {
            before,
            after: ticks_while_masked,
        };
    }

    // Unmask and require delivery to resume.
    unsafe { set_timer_masked(false) };
    let resumed = spin_until(PROOF_SPIN_BUDGET, || timer_ticks() > ticks_while_masked);
    let ticks_after_unmask = timer_ticks();

    unsafe {
        disable_interrupts();
        set_timer_masked(true);
    }

    if !resumed {
        return DeliveryProof::DidNotResume {
            ticks: ticks_after_unmask,
        };
    }

    DeliveryProof::Passed {
        ticks_after_run,
        ticks_while_masked,
        ticks_after_unmask,
    }
}
