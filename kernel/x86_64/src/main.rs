#![no_main]
#![no_std]
// The dedicated smoke-test images intentionally diverge inside `_start` before
// the normal boot continuation runs, leaving its helper functions unused in
// those builds only. The default (normal) build keeps full dead-code analysis.
#![cfg_attr(
    any(feature = "exception-smoke-test", feature = "double-fault-smoke-test"),
    allow(dead_code)
)]

mod acpi;
mod apic_timer;
mod device_irq;
mod interrupt_vectors;
mod interrupts;
mod ioapic;
mod irq_proof;
mod legacy_pic;
mod local_apic;
mod interrupt_stub;
mod memory_protection;
mod msi;
#[cfg(feature = "msi-proof-device")]
mod msi_proof;
mod pci_config;
mod pit;
mod security_baseline;
mod smp;
mod virtual_memory;

use irq_proof::DeliveryProof;
use memory_protection::{ProofOutcome, ProtectionProof};

use aw_kernel_core::{
    HANDOFF_FLAG_FRAMEBUFFER_PRESENT, HANDOFF_FLAG_PCIE_ECAM_PRESENT, HandoffPixelFormat,
    KernelHandoff, MemoryDescriptorHandoff, UEFI_MEMORY_TYPE_CONVENTIONAL,
};
use aw_memory::{BootstrapPageAllocator, PhysicalRange};
use aw_pci::{PciAddress, PciDeviceIdentity};
use aw_x86_platform::{
    CpuAddressWidths, CpuFeatures, CpuIdentity, CpuSignature, CpuVendor, CpuidRegisters,
};
use core::arch::asm;
use core::arch::x86_64::__cpuid_count;
use core::panic::PanicInfo;

const DEBUG_PORT: u16 = 0x00e9;
const PCI_CONFIG_ADDRESS_PORT: u16 = 0x0cf8;
const PCI_CONFIG_DATA_PORT: u16 = 0x0cfc;

#[inline(always)]
unsafe fn outb(port: u16, value: u8) {
    // SAFETY: The caller ensures the selected I/O port accepts byte writes.
    unsafe {
        asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

#[inline(always)]
unsafe fn outl(port: u16, value: u32) {
    // SAFETY: The caller ensures the selected I/O port accepts dword writes.
    unsafe {
        asm!(
            "out dx, eax",
            in("dx") port,
            in("eax") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

#[inline(always)]
unsafe fn inl(port: u16) -> u32 {
    let value: u32;
    // SAFETY: The caller ensures the selected I/O port accepts dword reads.
    unsafe {
        asm!(
            "in eax, dx",
            in("dx") port,
            out("eax") value,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

fn debug_write(message: &str) {
    for byte in message.bytes() {
        // SAFETY: DEBUG_PORT is the conventional byte-wide QEMU/Bochs debug port.
        unsafe { outb(DEBUG_PORT, byte) };
    }
}

fn debug_write_u8(mut value: u8) {
    let mut digits = [0_u8; 3];
    let mut index = digits.len();

    if value == 0 {
        // SAFETY: DEBUG_PORT is the conventional byte-wide QEMU/Bochs debug port.
        unsafe { outb(DEBUG_PORT, b'0') };
        return;
    }

    while value != 0 {
        index -= 1;
        digits[index] = b'0' + value % 10;
        value /= 10;
    }

    for byte in &digits[index..] {
        // SAFETY: DEBUG_PORT is the conventional byte-wide QEMU/Bochs debug port.
        unsafe { outb(DEBUG_PORT, *byte) };
    }
}

fn debug_write_u64(mut value: u64) {
    let mut digits = [0_u8; 20];
    let mut index = digits.len();

    if value == 0 {
        // SAFETY: DEBUG_PORT is the conventional byte-wide QEMU/Bochs debug port.
        unsafe { outb(DEBUG_PORT, b'0') };
        return;
    }

    while value != 0 {
        index -= 1;
        digits[index] = b'0' + (value % 10) as u8;
        value /= 10;
    }

    for byte in &digits[index..] {
        // SAFETY: DEBUG_PORT is the conventional byte-wide QEMU/Bochs debug port.
        unsafe { outb(DEBUG_PORT, *byte) };
    }
}

fn debug_write_hex_u64(value: u64) {
    debug_write("0x");
    for shift in (0..16).rev() {
        let nibble = ((value >> (shift * 4)) & 0x0f) as u8;
        let byte = if nibble < 10 {
            b'0' + nibble
        } else {
            b'a' + (nibble - 10)
        };
        // SAFETY: DEBUG_PORT is the conventional byte-wide QEMU/Bochs debug port.
        unsafe { outb(DEBUG_PORT, byte) };
    }
}
#[inline(always)]
fn halt_forever() -> ! {
    loop {
        // SAFETY: Interrupts are disabled before entering the kernel main path,
        // so HLT cannot dispatch into an uninitialized interrupt descriptor table.
        unsafe { asm!("hlt", options(nomem, nostack, preserves_flags)) };
    }
}

#[inline(always)]
fn cpuid(leaf: u32, subleaf: u32) -> CpuidRegisters {
    let registers = __cpuid_count(leaf, subleaf);
    CpuidRegisters {
        eax: registers.eax,
        ebx: registers.ebx,
        ecx: registers.ecx,
        edx: registers.edx,
    }
}

fn detect_cpu() -> CpuIdentity {
    let leaf0 = cpuid(0, 0);
    let max_basic_leaf = leaf0.eax;
    let extended_leaf0 = cpuid(0x8000_0000, 0);
    let max_extended_leaf = extended_leaf0.eax;

    let leaf1 = if max_basic_leaf >= 1 {
        cpuid(1, 0)
    } else {
        CpuidRegisters::ZERO
    };
    let leaf7 = (max_basic_leaf >= 7).then(|| cpuid(7, 0));
    let extended_leaf1 = (max_extended_leaf >= 0x8000_0001).then(|| cpuid(0x8000_0001, 0));
    let extended_leaf7 = (max_extended_leaf >= 0x8000_0007).then(|| cpuid(0x8000_0007, 0));
    let extended_leaf8 = (max_extended_leaf >= 0x8000_0008).then(|| cpuid(0x8000_0008, 0));

    CpuIdentity {
        vendor: CpuVendor::from_leaf0(leaf0),
        signature: CpuSignature::from_leaf1_eax(leaf1.eax),
        max_basic_leaf,
        max_extended_leaf,
        address_widths: CpuAddressWidths::from_extended_leaf8(extended_leaf8),
        features: CpuFeatures::from_leaves(leaf1, leaf7, extended_leaf1, extended_leaf7),
    }
}

fn debug_cpu_vendor(vendor: CpuVendor) {
    debug_write("AW_CPU_VENDOR_OK vendor=");
    match vendor {
        CpuVendor::Amd => debug_write("amd"),
        CpuVendor::Intel => debug_write("intel"),
        CpuVendor::Other(_) => debug_write("other"),
    }
    debug_write("\n");
}

fn debug_cpu_address_widths(widths: CpuAddressWidths) {
    debug_write("AW_CPU_ADDRESS_WIDTH_OK physical=");
    debug_write_u8(widths.physical);
    debug_write(" linear=");
    debug_write_u8(widths.linear);
    debug_write("\n");
}

fn validate_cpu_baseline() {
    let cpu = detect_cpu();

    debug_cpu_vendor(cpu.vendor);

    if cpu.features.meets_boot_baseline() {
        debug_write("AW_CPU_BASELINE_OK apic=1 sse2=1 long_mode=1\n");
    } else {
        debug_write("AW_CPU_BASELINE_FAIL\n");
        halt_forever();
    }

    if cpu.is_supported_vendor() {
        debug_write("AW_CPU_VENDOR_SUPPORTED\n");
    } else {
        debug_write("AW_CPU_VENDOR_GENERIC_FALLBACK\n");
    }

    if cpu.features.nx {
        debug_write("AW_CPU_NX_OK\n");
    } else {
        debug_write("AW_CPU_NX_FAIL\n");
        halt_forever();
    }

    debug_cpu_address_widths(cpu.address_widths);
    if cpu.meets_paging_baseline() {
        debug_write("AW_PAGING_BASELINE_OK\n");
    } else {
        debug_write("AW_PAGING_BASELINE_FAIL\n");
        halt_forever();
    }

    if cpu.features.x2apic {
        debug_write("AW_CPU_X2APIC_AVAILABLE\n");
    } else {
        debug_write("AW_CPU_X2APIC_FALLBACK_APIC\n");
    }

    if cpu.features.invariant_tsc {
        debug_write("AW_CPU_INVARIANT_TSC_AVAILABLE\n");
    } else {
        debug_write("AW_CPU_TIMER_FALLBACK_REQUIRED\n");
    }
}

fn memory_map_descriptors(handoff: &KernelHandoff) -> Option<&[MemoryDescriptorHandoff]> {
    let map = handoff.memory_map;
    if !map.is_valid() || map.buffer_address > usize::MAX as u64 {
        return None;
    }

    // SAFETY: The ABI v4 loader reserves the normalized descriptor buffer as
    // LOADER_DATA before ExitBootServices and transfers control without freeing
    // it. `MemoryMapHandoff::is_valid` verifies alignment, descriptor size and
    // byte length before this slice is constructed.
    Some(unsafe {
        core::slice::from_raw_parts(
            map.buffer_address as usize as *const MemoryDescriptorHandoff,
            map.entry_count as usize,
        )
    })
}

fn validate_memory_map(handoff: &KernelHandoff) -> bool {
    let Some(descriptors) = memory_map_descriptors(handoff) else {
        debug_write("AW_MEMORY_MAP_VALIDATE_FAIL reason=shape\n");
        return false;
    };

    let mut conventional_pages = 0_u64;
    for descriptor in descriptors {
        if !descriptor.is_valid() {
            debug_write("AW_MEMORY_MAP_VALIDATE_FAIL reason=descriptor\n");
            return false;
        }
        if descriptor.memory_type == UEFI_MEMORY_TYPE_CONVENTIONAL {
            let Some(total) = conventional_pages.checked_add(descriptor.page_count) else {
                debug_write("AW_MEMORY_MAP_VALIDATE_FAIL reason=page_overflow\n");
                return false;
            };
            conventional_pages = total;
        }
    }

    if conventional_pages == 0 {
        debug_write("AW_MEMORY_MAP_VALIDATE_FAIL reason=no_conventional_memory\n");
        return false;
    }

    debug_write("AW_MEMORY_MAP_VALIDATE_OK\n");
    debug_write("AW_MEMORY_MAP_CONVENTIONAL_OK\n");
    true
}

fn probe_bootstrap_page_allocator(handoff: &KernelHandoff) -> bool {
    let Some(descriptors) = memory_map_descriptors(handoff) else {
        debug_write("AW_BOOTSTRAP_PAGE_ALLOC_FAIL reason=memory_map\n");
        return false;
    };

    let kernel_image = handoff.kernel_image;
    let Some(kernel_end) = kernel_image.allocation_end_exclusive() else {
        debug_write("AW_KERNEL_RANGE_PROTECTED_FAIL reason=overflow\n");
        return false;
    };
    let Some(kernel_range) = PhysicalRange::new(kernel_image.physical_address, kernel_end) else {
        debug_write("AW_KERNEL_RANGE_PROTECTED_FAIL reason=shape\n");
        return false;
    };
    let protected_ranges = [kernel_range];

    let mut allocator =
        match BootstrapPageAllocator::with_protected_ranges(descriptors, &protected_ranges) {
            Ok(allocator) => allocator,
            Err(_) => {
                debug_write("AW_BOOTSTRAP_PAGE_ALLOC_FAIL reason=allocator_init\n");
                return false;
            }
        };

    let Some(page) = allocator.allocate_page() else {
        debug_write("AW_BOOTSTRAP_PAGE_ALLOC_FAIL reason=no_page\n");
        return false;
    };
    if kernel_range.contains_address(page.start_address()) {
        debug_write("AW_KERNEL_RANGE_PROTECTED_FAIL reason=allocated_kernel_page\n");
        return false;
    }

    debug_write("AW_KERNEL_RANGE_PROTECTED_OK\n");
    debug_write("AW_BOOTSTRAP_PAGE_ALLOC_OK\n");
    true
}

fn activate_virtual_memory(handoff: &KernelHandoff) -> Option<virtual_memory::ActiveMap> {
    debug_write("AW_VMM_BEGIN\n");

    let Some(descriptors) = memory_map_descriptors(handoff) else {
        debug_write("AW_VMM_FAIL reason=memory_map\n");
        return None;
    };

    let kernel_image = handoff.kernel_image;
    let Some(kernel_end) = kernel_image.allocation_end_exclusive() else {
        debug_write("AW_VMM_FAIL reason=kernel_range_overflow\n");
        return None;
    };
    let Some(kernel_range) = PhysicalRange::new(kernel_image.physical_address, kernel_end) else {
        debug_write("AW_VMM_FAIL reason=kernel_range_shape\n");
        return None;
    };
    let protected_ranges = [kernel_range];

    let mut allocator =
        match BootstrapPageAllocator::with_protected_ranges(descriptors, &protected_ranges) {
            Ok(allocator) => allocator,
            Err(_) => {
                debug_write("AW_VMM_FAIL reason=allocator_init\n");
                return None;
            }
        };

    // SAFETY: CPL0 single-core bootstrap after IDT/TSS install. Page-table
    // frames come from conventional RAM outside the kernel image, and the map
    // identity-covers the current RIP, stack, framebuffer and ECAM window, so
    // execution continues across the CR3 switch.
    let map = unsafe {
        virtual_memory::activate(
            || allocator.allocate_page().map(|page| page.start_address()),
            interrupts::double_fault_guard_page(),
        )
    };

    match map {
        Ok(map) => {
            debug_write("AW_VMM_CR3 prev=");
            debug_write_hex_u64(map.previous_cr3);
            debug_write(" new=");
            debug_write_hex_u64(map.cr3);
            debug_write("\n");
            debug_write("AW_VMM_IDENTITY_MAP_OK gib=");
            debug_write_u8(virtual_memory::IDENTITY_GIB as u8);
            debug_write(" tables=");
            debug_write_u64(map.table_count as u64);
            debug_write("\n");
            debug_write("AW_VMM_WX_LAYOUT text=");
            debug_write_hex_u64(map.layout.text.0);
            debug_write("..");
            debug_write_hex_u64(map.layout.text.1);
            debug_write(" data=");
            debug_write_hex_u64(map.layout.data.0);
            debug_write("..");
            debug_write_hex_u64(map.layout.data.1);
            debug_write(" guard=");
            debug_write_hex_u64(map.guard_page);
            debug_write("\n");
            debug_write("AW_VMM_ACTIVE\n");
            Some(map)
        }
        Err(error) => {
            debug_write("AW_VMM_FAIL reason=");
            debug_write(error.name());
            debug_write("\n");
            None
        }
    }
}

/// Enable the CPU protection bits, then prove each one with a real fault.
///
/// Nothing here is claimed from a flag or a build success: every protection is
/// asserted only after the CPU has actually refused the corresponding access
/// with the expected `#PF` error code (dossier section 7 and DOD-03).
fn prove_memory_protections(map: &virtual_memory::ActiveMap) {
    debug_write("AW_SECURITY_BASELINE_BEGIN\n");
    // SAFETY: CPL0, single-core bootstrap, after the kernel owns its page
    // tables and with no user-accessible page mapped anywhere.
    let state = unsafe { security_baseline::enforce_baseline() };
    debug_write("AW_SECURITY_ENFORCED wp=");
    debug_write_u8(u8::from(state.cr0_write_protect));
    debug_write(" nx=");
    debug_write_u8(u8::from(state.efer_nx_enable));
    debug_write(" smep=");
    debug_write_u8(u8::from(state.cr4_smep));
    debug_write(" smap=");
    debug_write_u8(u8::from(state.cr4_smap));
    debug_write(" umip=");
    debug_write_u8(u8::from(state.cr4_umip));
    debug_write("\n");

    // SAFETY: CPL0, read-only.
    match unsafe { security_baseline::first_security_gap() } {
        None => debug_write("AW_SECURITY_BASELINE_OK\n"),
        Some(gap) => {
            debug_write("AW_SECURITY_BASELINE_GAP reason=");
            debug_write(gap.name());
            debug_write("\n");
        }
    }

    debug_write("AW_MEMORY_PROTECTION_BEGIN\n");

    // Each probe targets a page whose permissions the map audit already
    // verified, so a missing fault means the CPU is not enforcing them.
    // SAFETY: every target is a kernel-owned page of the running image; the
    // probes recover through the armed exception path and never run foreign
    // code or corrupt live data.
    // The guard page is the first page of `.data`, so the NX probes target
    // pages that are definitely present: otherwise a not-present fault would
    // masquerade as an NX fault and prove nothing about NX.
    let writable_page = map.layout.data.1 - 4096;
    let proofs = [
        ProtectionProof {
            name: "nx-execute-rodata",
            outcome: unsafe { memory_protection::probe_execute_data(map.layout.rodata.0) },
        },
        ProtectionProof {
            name: "nx-execute-data",
            outcome: unsafe { memory_protection::probe_execute_data(writable_page) },
        },
        ProtectionProof {
            name: "wx-write-text",
            outcome: unsafe { memory_protection::probe_write_readonly(map.layout.text.0) },
        },
        ProtectionProof {
            name: "guard-page",
            outcome: unsafe { memory_protection::probe_touch_guard_page(map.guard_page) },
        },
    ];

    let all_passed = proofs.iter().all(|proof| proof.outcome.is_pass());
    for proof in proofs {
        match proof.outcome {
            ProofOutcome::Faulted(fault) => {
                debug_write("AW_MEMORY_PROTECTION_OK name=");
                debug_write(proof.name);
                debug_write(" error_code=");
                debug_write_hex_u64(fault.error_code);
                debug_write(" address=");
                debug_write_hex_u64(fault.address);
                debug_write("\n");
            }
            ProofOutcome::NoFault => {
                debug_write("AW_MEMORY_PROTECTION_FAIL name=");
                debug_write(proof.name);
                debug_write(" reason=no-fault\n");
            }
            ProofOutcome::WrongErrorCode(fault) => {
                debug_write("AW_MEMORY_PROTECTION_FAIL name=");
                debug_write(proof.name);
                debug_write(" reason=wrong-error-code error_code=");
                debug_write_hex_u64(fault.error_code);
                debug_write("\n");
            }
        }
    }

    if all_passed {
        debug_write("AW_MEMORY_PROTECTION_PROOF_OK\n");
    }
}

/// Number of timer interrupts the delivery proof requires before it accepts
/// that hardware interrupt delivery works. More than one, so a single spurious
/// or self-inflicted entry cannot pass it.
const APIC_TIMER_REQUIRED_TICKS: u64 = 8;

/// Run the Local APIC timer delivery proof and report it on the debug console.
///
/// Returns normally whether or not the proof passes: an absent or broken timer
/// must not stop the rest of bootstrap from reporting its own state, and the
/// markers make the failure explicit instead of silent.
fn prove_apic_timer_delivery() {
    debug_write("AW_APIC_TIMER_BEGIN\n");

    // SAFETY: runs once after IDT/TSS install, with interrupts still masked.
    // The legacy PIC is remapped and fully masked before the first `sti`, so an
    // unremapped IRQ0 cannot alias the #DF vector.
    if let Err(reason) = unsafe { apic_timer::arm_periodic_after_idt() } {
        debug_write("AW_APIC_TIMER_UNAVAILABLE reason=");
        debug_write(reason);
        debug_write("\n");
        return;
    }
    // SAFETY: CPL0; the PIC is reprogrammed and fully masked before any `sti`.
    unsafe { legacy_pic::remap_and_mask_all() };
    debug_write("AW_APIC_TIMER_ARMED mode=periodic vector=");
    debug_write_u8(interrupt_vectors::APIC_TIMER_VECTOR);
    debug_write("\n");

    // SAFETY: CPL0, immediately after arming. Returns with interrupts disabled
    // and the timer masked, so the idle loop keeps its documented invariant.
    let proof = unsafe { apic_timer::run_delivery_proof(APIC_TIMER_REQUIRED_TICKS) };

    match proof {
        DeliveryProof::Passed {
            ticks_after_run,
            ticks_while_masked,
            ticks_after_unmask,
        } => {
            debug_write("AW_APIC_TIMER_FIRED\n");
            debug_write("AW_APIC_TIMER_MONOTONIC_OK ticks=");
            debug_write_u64(ticks_after_run);
            debug_write(" required=");
            debug_write_u64(APIC_TIMER_REQUIRED_TICKS);
            debug_write("\n");
            debug_write("AW_APIC_TIMER_MASKED_STOPPED ticks=");
            debug_write_u64(ticks_while_masked);
            debug_write("\n");
            debug_write("AW_APIC_TIMER_UNMASKED_RESUMED ticks=");
            debug_write_u64(ticks_after_unmask);
            debug_write("\n");
            debug_write("AW_APIC_TIMER_DELIVERY_PROOF_OK\n");
        }
        DeliveryProof::NotDelivered { ticks } => {
            debug_write("AW_APIC_TIMER_NOT_FIRED ticks=");
            debug_write_u64(ticks);
            debug_write("\n");
        }
        DeliveryProof::MaskIneffective { before, after } => {
            // The counter moved with the vector masked, so the increments are
            // not attributable to real interrupt delivery.
            debug_write("AW_APIC_TIMER_MASK_INEFFECTIVE before=");
            debug_write_u64(before);
            debug_write(" after=");
            debug_write_u64(after);
            debug_write("\n");
        }
        DeliveryProof::DidNotResume { ticks } => {
            debug_write("AW_APIC_TIMER_DID_NOT_RESUME ticks=");
            debug_write_u64(ticks);
            debug_write("\n");
        }
    }
}

/// Route a real device interrupt through an I/O APIC and prove it arrives.
///
/// Unlike the local APIC timer, nothing on this path is internal to the CPU:
/// the 8254 drives an interrupt pin, the I/O APIC translates that pin into the
/// vector its redirection entry names, and only then does the CPU see anything.
/// Which pin is not guessed - the MADT's interrupt source overrides decide it,
/// and on most platforms ISA IRQ 0 is not global system interrupt 0.
fn prove_device_interrupt_routing(handoff: &KernelHandoff) {
    debug_write("AW_IOAPIC_BEGIN\n");

    // SAFETY: the identity map is active and the RSDP address comes from the
    // handoff the loader already validated.
    let madt = match unsafe { acpi::find_madt(handoff.acpi_rsdp) } {
        Ok(madt) => madt,
        Err(error) => {
            debug_write("AW_IOAPIC_UNAVAILABLE reason=madt_");
            debug_write(error.name());
            debug_write("\n");
            return;
        }
    };

    debug_write("AW_MADT_OK local_apic=");
    debug_write_hex_u64(u64::from(madt.local_apic_address()));
    debug_write(" dual_8259=");
    debug_write_u8(u8::from(madt.dual_8259_present()));
    debug_write("\n");

    // SAFETY: CPL0 with interrupts disabled, after the IDT is installed and the
    // local APIC is running in x2APIC mode.
    let routed = match unsafe { device_irq::route_pit_through_ioapic(madt) } {
        Ok(routed) => routed,
        Err(reason) => {
            debug_write("AW_IOAPIC_UNAVAILABLE reason=");
            debug_write(reason);
            debug_write("\n");
            return;
        }
    };

    let routing = routed.routing;
    debug_write("AW_IOAPIC_FOUND id=");
    debug_write_u8(routing.io_apic_id);
    debug_write(" madt_id=");
    debug_write_u8(routing.madt_id);
    debug_write(" base=");
    debug_write_hex_u64(routing.base);
    debug_write(" entries=");
    debug_write_u64(u64::from(routing.entry_count));
    debug_write("\n");
    debug_write("AW_IOAPIC_ROUTED isa_irq=");
    debug_write_u8(device_irq::PIT_ISA_IRQ);
    debug_write(" gsi=");
    debug_write_u64(u64::from(routing.global_system_interrupt));
    debug_write(" index=");
    debug_write_u64(u64::from(routing.redirection_index));
    debug_write(" vector=");
    debug_write_hex_u64(u64::from(routing.vector));
    debug_write("\n");

    // SAFETY: CPL0 on a route this function just programmed, with the legacy
    // PIC already remapped and fully masked by the APIC timer proof.
    match unsafe { device_irq::prove_routed_delivery(&routed) } {
        DeliveryProof::Passed {
            ticks_after_run,
            ticks_while_masked,
            ticks_after_unmask,
        } => {
            debug_write("AW_IOAPIC_IRQ_FIRED\n");
            debug_write("AW_IOAPIC_IRQ_MONOTONIC_OK ticks=");
            debug_write_u64(ticks_after_run);
            debug_write(" required=");
            debug_write_u64(device_irq::REQUIRED_TICKS);
            debug_write("\n");
            debug_write("AW_IOAPIC_MASKED_STOPPED ticks=");
            debug_write_u64(ticks_while_masked);
            debug_write("\n");
            debug_write("AW_IOAPIC_UNMASKED_RESUMED ticks=");
            debug_write_u64(ticks_after_unmask);
            debug_write("\n");
            debug_write("AW_IOAPIC_DELIVERY_PROOF_OK\n");
        }
        DeliveryProof::NotDelivered { ticks } => {
            debug_write("AW_IOAPIC_IRQ_NOT_FIRED ticks=");
            debug_write_u64(ticks);
            debug_write("\n");
        }
        DeliveryProof::MaskIneffective { before, after } => {
            debug_write("AW_IOAPIC_MASK_INEFFECTIVE before=");
            debug_write_u64(before);
            debug_write(" after=");
            debug_write_u64(after);
            debug_write("\n");
        }
        DeliveryProof::DidNotResume { ticks } => {
            debug_write("AW_IOAPIC_DID_NOT_RESUME ticks=");
            debug_write_u64(ticks);
            debug_write("\n");
        }
    }
}

fn pci_read_u32(address: PciAddress, register_offset: u8) -> Option<u32> {
    let config_address = address.mechanism1_address(register_offset)?;
    // SAFETY: PCI configuration mechanism #1 uses the architected CF8/CFC
    // dword I/O ports. This is retained as a compatibility fallback.
    unsafe {
        outl(PCI_CONFIG_ADDRESS_PORT, config_address);
        Some(inl(PCI_CONFIG_DATA_PORT))
    }
}

/// Start the application processors and prove each one really runs, on tables
/// of its own.
///
/// "Online" is not claimed from a counter the bootstrap processor increments.
/// Each AP reports the APIC ID it read from its *own* local APIC, plus the GDT,
/// TSS and IST1 addresses it actually loaded - values no other CPU could have
/// produced, and which must all differ from each other and from the bootstrap
/// processor's.
fn bring_up_secondary_processors(handoff: &KernelHandoff) {
    debug_write("AW_SMP_BEGIN\n");

    // SAFETY: the identity map is active and the RSDP comes from the validated
    // handoff.
    let madt = match unsafe { acpi::find_madt(handoff.acpi_rsdp) } {
        Ok(madt) => madt,
        Err(error) => {
            debug_write("AW_SMP_UNAVAILABLE reason=madt_");
            debug_write(error.name());
            debug_write("\n");
            return;
        }
    };

    // SAFETY: CPL0 on the bootstrap processor, after the IDT is installed, the
    // kernel owns its page tables and x2APIC is enabled.
    let result = match unsafe { smp::bring_up(handoff, madt) } {
        Ok(result) => result,
        Err(reason) => {
            debug_write("AW_SMP_UNAVAILABLE reason=");
            debug_write(reason);
            debug_write("\n");
            return;
        }
    };

    debug_write("AW_SMP_CPUS described=");
    debug_write_u64(result.described as u64);
    debug_write(" bsp_apic_id=");
    debug_write_u64(u64::from(result.bootstrap_apic_id));
    debug_write("\n");

    let (bsp_gdt, bsp_tss) = interrupts::bootstrap_tables();
    let mut tables_are_private = result.online > 0;
    let mut seen: [(u64, u64, u32); interrupts::MAX_CPUS] = [(0, 0, 0); interrupts::MAX_CPUS];
    let mut seen_count = 0;

    for cpu in 1..interrupts::MAX_CPUS {
        let Some(summary) = smp::ap_summary(cpu) else {
            continue;
        };

        debug_write("AW_SMP_AP_ONLINE cpu=");
        debug_write_u64(summary.cpu as u64);
        debug_write(" apic_id=");
        debug_write_u64(u64::from(summary.reported_apic_id));
        debug_write(" requested=");
        debug_write_u64(u64::from(summary.requested_apic_id));
        debug_write(" tr=");
        debug_write_hex_u64(u64::from(summary.task_register));
        debug_write(" gdt=");
        debug_write_hex_u64(summary.gdt_base);
        debug_write(" tss=");
        debug_write_hex_u64(summary.tss_base);
        debug_write(" ist1=");
        debug_write_hex_u64(summary.ist1_top);
        debug_write("\n");

        // The AP that answered must be the one that was asked, and it must not
        // be sharing a descriptor table with anyone.
        if summary.reported_apic_id != summary.requested_apic_id
            || summary.gdt_base == bsp_gdt
            || summary.tss_base == bsp_tss
            || seen[..seen_count].iter().any(|&(gdt, tss, apic_id)| {
                gdt == summary.gdt_base
                    || tss == summary.tss_base
                    || apic_id == summary.reported_apic_id
            })
        {
            tables_are_private = false;
        }

        seen[seen_count] = (
            summary.gdt_base,
            summary.tss_base,
            summary.reported_apic_id,
        );
        seen_count += 1;
    }

    debug_write("AW_SMP_ONLINE online=");
    debug_write_u64(result.online as u64);
    debug_write(" started=");
    debug_write_u64(result.started as u64);
    debug_write("\n");

    if result.started == 0 {
        debug_write("AW_SMP_NO_APPLICATION_PROCESSORS\n");
        return;
    }
    if result.online != result.started {
        debug_write("AW_SMP_AP_NOT_ONLINE\n");
        return;
    }
    if !tables_are_private {
        debug_write("AW_SMP_TABLES_SHARED\n");
        return;
    }

    debug_write("AW_SMP_PER_CPU_TABLES_OK cpus=");
    debug_write_u64(seen_count as u64);
    debug_write("\n");
    debug_write("AW_SMP_ALL_ONLINE\n");
}

/// Prove an MSI arrives: the device writes the interrupt into the local APIC
/// itself, with no pin and no I/O APIC anywhere in the path.
///
/// Only built with `msi-proof-device`, which also pulls in the driver for the
/// emulator test device this drives.
#[cfg(feature = "msi-proof-device")]
fn prove_msi_delivery(handoff: &KernelHandoff) {
    debug_write("AW_MSI_BEGIN\n");

    // SAFETY: CPL0 with interrupts disabled, after the IDT is installed and the
    // local APIC is running in x2APIC mode.
    let device = match unsafe { msi_proof::program_msi_device(handoff) } {
        Ok(device) => device,
        Err(reason) => {
            debug_write("AW_MSI_UNAVAILABLE reason=");
            debug_write(reason);
            debug_write("\n");
            return;
        }
    };

    debug_write("AW_MSI_DEVICE_FOUND bus=");
    debug_write_u8(device.bus);
    debug_write(" device=");
    debug_write_u8(device.device);
    debug_write(" function=");
    debug_write_u8(device.function);
    debug_write("\n");
    debug_write("AW_MSI_PROGRAMMED vector=");
    debug_write_hex_u64(u64::from(device.vector));
    debug_write(" address=");
    debug_write_hex_u64(u64::from(device.message.address));
    debug_write(" data=");
    debug_write_hex_u64(u64::from(device.message.data));
    debug_write("\n");

    // SAFETY: CPL0 on a device this function just programmed.
    match unsafe { msi_proof::prove_msi_delivery(&device) } {
        DeliveryProof::Passed {
            ticks_after_run,
            ticks_while_masked,
            ticks_after_unmask,
        } => {
            debug_write("AW_MSI_FIRED\n");
            debug_write("AW_MSI_MONOTONIC_OK ticks=");
            debug_write_u64(ticks_after_run);
            debug_write(" required=");
            debug_write_u64(msi_proof::REQUIRED_TICKS);
            debug_write("\n");
            debug_write("AW_MSI_MASKED_STOPPED ticks=");
            debug_write_u64(ticks_while_masked);
            debug_write("\n");
            debug_write("AW_MSI_UNMASKED_RESUMED ticks=");
            debug_write_u64(ticks_after_unmask);
            debug_write("\n");
            debug_write("AW_MSI_DELIVERY_PROOF_OK\n");
        }
        DeliveryProof::NotDelivered { ticks } => {
            debug_write("AW_MSI_NOT_FIRED ticks=");
            debug_write_u64(ticks);
            debug_write("\n");
        }
        DeliveryProof::MaskIneffective { before, after } => {
            debug_write("AW_MSI_MASK_INEFFECTIVE before=");
            debug_write_u64(before);
            debug_write(" after=");
            debug_write_u64(after);
            debug_write("\n");
        }
        DeliveryProof::DidNotResume { ticks } => {
            debug_write("AW_MSI_DID_NOT_RESUME ticks=");
            debug_write_u64(ticks);
            debug_write("\n");
        }
    }
}

fn classify_pci_device(identity: PciDeviceIdentity, found: &mut [bool; 4]) {
    if identity.class.is_nvme() {
        found[0] = true;
    }
    if identity.class.is_ahci() {
        found[1] = true;
    }
    if identity.class.is_xhci() {
        found[2] = true;
    }
    if identity.class.is_hda() {
        found[3] = true;
    }
}

fn emit_pci_classes(found: [bool; 4]) {
    if found[0] {
        debug_write("AW_PCI_NVME_FOUND\n");
    }
    if found[1] {
        debug_write("AW_PCI_AHCI_FOUND\n");
    }
    if found[2] {
        debug_write("AW_PCI_XHCI_FOUND\n");
    }
    if found[3] {
        debug_write("AW_PCI_HDA_FOUND\n");
    }
}

fn scan_pcie_ecam(handoff: &KernelHandoff) -> bool {
    if handoff.flags & HANDOFF_FLAG_PCIE_ECAM_PRESENT == 0 || handoff.pcie_ecam_count == 0 {
        return false;
    }

    debug_write("AW_PCIE_ECAM_SCAN_BEGIN\n");
    let mut any_device = false;
    let mut found = [false; 4];

    for region in handoff
        .pcie_ecam
        .iter()
        .take(handoff.pcie_ecam_count as usize)
        .copied()
    {
        for bus in region.start_bus..=region.end_bus {
            for device in 0_u8..32 {
                let vendor_device0 =
                    pci_config::read_u32(region, bus, device, 0, 0x00).unwrap_or(u32::MAX);
                if vendor_device0 as u16 == 0xffff {
                    continue;
                }

                let header_register = pci_config::read_u32(region, bus, device, 0, 0x0c).unwrap_or(0);
                let header_type = ((header_register >> 16) & 0xff) as u8;
                let function_count = if header_type & 0x80 != 0 { 8 } else { 1 };

                for function in 0_u8..function_count {
                    let vendor_device =
                        pci_config::read_u32(region, bus, device, function, 0x00).unwrap_or(u32::MAX);
                    if vendor_device as u16 == 0xffff {
                        continue;
                    }

                    let class_revision =
                        pci_config::read_u32(region, bus, device, function, 0x08).unwrap_or(0);
                    let subsystem = pci_config::read_u32(region, bus, device, function, 0x2c);
                    let identity = PciDeviceIdentity::from_config_registers(
                        vendor_device,
                        class_revision,
                        subsystem,
                    );
                    any_device = true;
                    classify_pci_device(identity, &mut found);
                }
            }
        }
    }

    if any_device {
        debug_write("AW_PCIE_ECAM_SCAN_OK\n");
        emit_pci_classes(found);
    } else {
        debug_write("AW_PCIE_ECAM_SCAN_EMPTY\n");
    }
    any_device
}

fn scan_pci_mechanism1() {
    debug_write("AW_PCI_SCAN_BEGIN mechanism=cf8_cfc\n");

    let mut any_device = false;
    let mut found = [false; 4];

    for bus in 0_u16..=255 {
        for device in 0_u8..32 {
            let function0 = match PciAddress::new(0, bus as u8, device, 0) {
                Some(address) => address,
                None => continue,
            };
            let vendor_device0 = pci_read_u32(function0, 0x00).unwrap_or(u32::MAX);
            if vendor_device0 as u16 == 0xffff {
                continue;
            }

            let header_register = pci_read_u32(function0, 0x0c).unwrap_or(0);
            let header_type = ((header_register >> 16) & 0xff) as u8;
            let function_count = if header_type & 0x80 != 0 { 8 } else { 1 };

            for function in 0_u8..function_count {
                let address = match PciAddress::new(0, bus as u8, device, function) {
                    Some(value) => value,
                    None => continue,
                };
                let vendor_device = pci_read_u32(address, 0x00).unwrap_or(u32::MAX);
                if vendor_device as u16 == 0xffff {
                    continue;
                }

                let class_revision = pci_read_u32(address, 0x08).unwrap_or(0);
                let subsystem = pci_read_u32(address, 0x2c);
                let identity = PciDeviceIdentity::from_config_registers(
                    vendor_device,
                    class_revision,
                    subsystem,
                );
                any_device = true;
                classify_pci_device(identity, &mut found);
            }
        }
    }

    if !any_device {
        debug_write("AW_PCI_SCAN_FAIL reason=no_devices\n");
        return;
    }

    debug_write("AW_PCI_SCAN_OK\n");
    emit_pci_classes(found);
}

fn scan_pci(handoff: &KernelHandoff) {
    if scan_pcie_ecam(handoff) {
        debug_write("AW_PCI_TRANSPORT_OK transport=ecam\n");
    } else {
        debug_write("AW_PCI_TRANSPORT_FALLBACK transport=cf8_cfc\n");
        scan_pci_mechanism1();
    }
}

fn paint_boot_marker(handoff: &KernelHandoff) -> bool {
    if handoff.flags & HANDOFF_FLAG_FRAMEBUFFER_PRESENT == 0 {
        return false;
    }

    let framebuffer = handoff.framebuffer;
    if !matches!(
        framebuffer.pixel_format,
        HandoffPixelFormat::Rgb | HandoffPixelFormat::Bgr
    ) {
        return false;
    }

    let required_bytes = u64::from(framebuffer.stride_pixels)
        .saturating_mul(u64::from(framebuffer.height))
        .saturating_mul(4);
    if required_bytes > framebuffer.byte_len || framebuffer.physical_address == 0 {
        return false;
    }

    let rows = framebuffer.height.min(32);
    let width = framebuffer.width;
    let stride = u64::from(framebuffer.stride_pixels);
    let base = framebuffer.physical_address as *mut u32;

    for row in 0..rows {
        for column in 0..width {
            let pixel_index = u64::from(row)
                .saturating_mul(stride)
                .saturating_add(u64::from(column));
            if pixel_index >= framebuffer.byte_len / 4 {
                return false;
            }

            // SAFETY: The handoff was validated, bounds are checked above, and
            // this range is the UEFI-provided linear framebuffer.
            unsafe {
                core::ptr::write_volatile(base.add(pixel_index as usize), 0x00ff_00ff);
            }
        }
    }

    true
}

/// Native kernel entry point invoked by the UEFI loader after
/// `ExitBootServices`.
///
/// # Safety
///
/// The loader must pass a `handoff_ptr` that is either null or points at a
/// live, correctly initialized [`KernelHandoff`] that outlives this call. The
/// pointer is validated before any field other than nullness is trusted.
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text._start")]
pub unsafe extern "sysv64" fn _start(handoff_ptr: *const KernelHandoff) -> ! {
    // SAFETY: This is the first native kernel instruction path. No IDT exists
    // yet, so mask interrupts before touching any other CPU state.
    unsafe { asm!("cli", options(nomem, nostack, preserves_flags)) };

    if handoff_ptr.is_null() {
        debug_write("AW_NATIVE_KERNEL_FAIL reason=null_handoff\n");
        halt_forever();
    }

    // SAFETY: The UEFI loader constructs a KernelHandoff on its live stack and
    // transfers control without returning. The pointer remains valid here.
    let handoff = unsafe { &*handoff_ptr };
    if let Err(_error) = handoff.validate() {
        debug_write("AW_NATIVE_KERNEL_FAIL reason=invalid_handoff\n");
        halt_forever();
    }

    // SAFETY: _start masks interrupts before reaching this point. The tables
    // are installed once during single-core bootstrap.
    debug_write("AW_NATIVE_GDT_IDT_BEGIN\n");
    // `install()` emits AW_NATIVE_GDT_IDT_INSTALLED once the tables are live.
    unsafe { interrupts::install() };
    debug_write("AW_NATIVE_KERNEL_ENTRY_OK\n");

    // The three configurations below are mutually exclusive and each diverges,
    // so exactly one of them is the terminal path of `_start` in any given
    // build. Keeping the normal-boot continuation inside its own branch avoids
    // dead trailing code in the dedicated smoke-test images.
    #[cfg(feature = "double-fault-smoke-test")]
    {
        debug_write("AW_DOUBLE_FAULT_SMOKE_REQUEST\n");
        // SAFETY: dedicated smoke-test build; this intentionally causes a
        // delivery-time #GP failure so the CPU must enter #DF on IST1.
        unsafe { interrupts::trigger_double_fault_smoke() }
    }

    #[cfg(all(
        feature = "exception-smoke-test",
        not(feature = "double-fault-smoke-test")
    ))]
    {
        debug_write("AW_EXCEPTION_SMOKE_TRIGGER vector=6\n");
        // SAFETY: dedicated smoke-test build; `ud2` deterministically raises
        // #UD (vector 6) so the invalid-opcode handler can be observed.
        unsafe { asm!("ud2", options(noreturn)) }
    }

    #[cfg(not(any(feature = "exception-smoke-test", feature = "double-fault-smoke-test")))]
    {
        // Take over paging from the firmware before anything else in the
        // normal boot path, so the remaining bring-up runs on kernel-owned,
        // W^X page tables. Identity-mapped, so a failure here is non-fatal:
        // the firmware tables stay active and boot continues, with the
        // protections explicitly reported as unproven.
        match activate_virtual_memory(handoff) {
            Some(map) => prove_memory_protections(&map),
            None => debug_write("AW_MEMORY_PROTECTION_SKIPPED reason=no-kernel-page-tables\n"),
        }

        prove_apic_timer_delivery();
        prove_device_interrupt_routing(handoff);
        bring_up_secondary_processors(handoff);
        #[cfg(feature = "msi-proof-device")]
        prove_msi_delivery(handoff);

        if !validate_memory_map(handoff) {
            halt_forever();
        }
        if !probe_bootstrap_page_allocator(handoff) {
            halt_forever();
        }
        validate_cpu_baseline();
        scan_pci(handoff);

        if paint_boot_marker(handoff) {
            debug_write("AW_NATIVE_FRAMEBUFFER_WRITE_OK\n");
        } else {
            debug_write("AW_NATIVE_FRAMEBUFFER_WRITE_SKIP\n");
        }

        debug_write("AW_NATIVE_KERNEL_IDLE\n");
        halt_forever()
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    debug_write("AW_NATIVE_KERNEL_PANIC\n");
    halt_forever();
}

