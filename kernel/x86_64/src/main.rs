#![no_main]
#![no_std]
// The dedicated smoke-test images intentionally diverge inside `_start` before
// the normal boot continuation runs, leaving its helper functions unused in
// those builds only. The default (normal) build keeps full dead-code analysis.
#![cfg_attr(
    any(feature = "exception-smoke-test", feature = "double-fault-smoke-test"),
    allow(dead_code)
)]

mod apic_timer_irq_v25;
mod interrupts;
mod legacy_pic;
mod security_baseline_v26;
mod virtual_memory_v27;

use aw_kernel_core::{
    HANDOFF_FLAG_FRAMEBUFFER_PRESENT, HANDOFF_FLAG_PCIE_ECAM_PRESENT, HandoffPixelFormat,
    KernelHandoff, MemoryDescriptorHandoff, PciEcamHandoff, UEFI_MEMORY_TYPE_CONVENTIONAL,
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

fn activate_virtual_memory(handoff: &KernelHandoff) -> bool {
    debug_write("AW_VMM_BEGIN\n");

    if !virtual_memory_v27::supports_1gib_pages() {
        debug_write("AW_VMM_UNAVAILABLE reason=no-1gib-pages\n");
        return false;
    }

    let Some(descriptors) = memory_map_descriptors(handoff) else {
        debug_write("AW_VMM_FAIL reason=memory_map\n");
        return false;
    };

    let kernel_image = handoff.kernel_image;
    let Some(kernel_end) = kernel_image.allocation_end_exclusive() else {
        debug_write("AW_VMM_FAIL reason=kernel_range_overflow\n");
        return false;
    };
    let Some(kernel_range) = PhysicalRange::new(kernel_image.physical_address, kernel_end) else {
        debug_write("AW_VMM_FAIL reason=kernel_range_shape\n");
        return false;
    };
    let protected_ranges = [kernel_range];

    let mut allocator =
        match BootstrapPageAllocator::with_protected_ranges(descriptors, &protected_ranges) {
            Ok(allocator) => allocator,
            Err(_) => {
                debug_write("AW_VMM_FAIL reason=allocator_init\n");
                return false;
            }
        };

    let Some(pml4) = allocator.allocate_page() else {
        debug_write("AW_VMM_FAIL reason=no_pml4_frame\n");
        return false;
    };
    let Some(pdpt) = allocator.allocate_page() else {
        debug_write("AW_VMM_FAIL reason=no_pdpt_frame\n");
        return false;
    };

    let previous_cr3 = virtual_memory_v27::current_cr3();

    // SAFETY: CPL0 single-core bootstrap after IDT/TSS install. The two frames
    // are distinct, page-aligned conventional-RAM pages inside the low identity
    // window, so identity mapping keeps the current RIP, stack, framebuffer and
    // ECAM window valid across the CR3 switch.
    match unsafe {
        virtual_memory_v27::activate_identity_map(pml4.start_address(), pdpt.start_address())
    } {
        Ok(new_cr3) => {
            debug_write("AW_VMM_CR3 prev=");
            debug_write_hex_u64(previous_cr3);
            debug_write(" new=");
            debug_write_hex_u64(new_cr3);
            debug_write("\n");
            debug_write("AW_VMM_IDENTITY_MAP_OK gib=");
            debug_write_u8(virtual_memory_v27::IDENTITY_GIB as u8);
            debug_write("\n");
            debug_write("AW_VMM_ACTIVE\n");
            true
        }
        Err(error) => {
            debug_write("AW_VMM_FAIL reason=");
            // Inline literals stay RIP-relative; see debug_exception_name.
            match error {
                virtual_memory_v27::VmmError::NoOneGibPages => debug_write("no-1gib-pages"),
                virtual_memory_v27::VmmError::BadTableFrame => debug_write("bad-table-frame"),
                virtual_memory_v27::VmmError::BuildFailed => debug_write("build-failed"),
            }
            debug_write("\n");
            false
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

fn pcie_ecam_read_u32(
    region: PciEcamHandoff,
    bus: u8,
    device: u8,
    function: u8,
    register_offset: u16,
) -> Option<u32> {
    if !region.is_valid()
        || bus < region.start_bus
        || bus > region.end_bus
        || device > 31
        || function > 7
        || register_offset > 0x0ffc
        || register_offset & 3 != 0
    {
        return None;
    }

    let relative_bus = u64::from(bus - region.start_bus);
    let offset = (relative_bus << 20)
        | (u64::from(device) << 15)
        | (u64::from(function) << 12)
        | u64::from(register_offset);
    let address = region.base_address.checked_add(offset)?;
    if address > usize::MAX as u64 {
        return None;
    }

    // SAFETY: The region was validated from ACPI MCFG before ExitBootServices.
    // ECAM configuration registers are MMIO and are read using volatile access.
    // The bootstrap kernel is still executing with the firmware-established
    // physical-address mappings; the future VMM must explicitly preserve/map
    // these regions before replacing those page tables.
    Some(unsafe { core::ptr::read_volatile(address as usize as *const u32) })
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
                    pcie_ecam_read_u32(region, bus, device, 0, 0x00).unwrap_or(u32::MAX);
                if vendor_device0 as u16 == 0xffff {
                    continue;
                }

                let header_register = pcie_ecam_read_u32(region, bus, device, 0, 0x0c).unwrap_or(0);
                let header_type = ((header_register >> 16) & 0xff) as u8;
                let function_count = if header_type & 0x80 != 0 { 8 } else { 1 };

                for function in 0_u8..function_count {
                    let vendor_device =
                        pcie_ecam_read_u32(region, bus, device, function, 0x00).unwrap_or(u32::MAX);
                    if vendor_device as u16 == 0xffff {
                        continue;
                    }

                    let class_revision =
                        pcie_ecam_read_u32(region, bus, device, function, 0x08).unwrap_or(0);
                    let subsystem = pcie_ecam_read_u32(region, bus, device, function, 0x2c);
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
        // normal boot path, so the remaining bring-up runs on kernel-owned
        // page tables. Identity-mapped, so a failure here is non-fatal: the
        // firmware tables stay active and boot continues.
        activate_virtual_memory(handoff);

        debug_write("AW_SECURITY_BASELINE_BEGIN\n");
        // SAFETY: CPL0, single-core bootstrap, runs after IDT install.
        match unsafe { security_baseline_v26::first_security_gap() } {
            None => debug_write("AW_SECURITY_BASELINE_OK\n"),
            Some(gap) => {
                use security_baseline_v26::RuntimeSecurityGap as Gap;
                debug_write("AW_SECURITY_BASELINE_GAP reason=");
                // Inline literals keep the reason RIP-relative; a `&str`-returning
                // accessor would compile to a base-0 rodata pointer and print
                // blank in this flat, relocation-free kernel image.
                match gap {
                    Gap::WriteProtectDisabled => debug_write("cr0-write-protect-disabled"),
                    Gap::NxSupportedButDisabled => debug_write("nx-supported-but-disabled"),
                    Gap::SmepSupportedButDisabled => debug_write("smep-supported-but-disabled"),
                    Gap::SmapSupportedButDisabled => debug_write("smap-supported-but-disabled"),
                    Gap::UmipSupportedButDisabled => debug_write("umip-supported-but-disabled"),
                }
                debug_write("\n");
            }
        }

        debug_write("AW_APIC_TIMER_BEGIN\n");
        // SAFETY: runs once after IDT/TSS install, with interrupts still
        // masked. The legacy PIC is remapped and fully masked before the
        // first `sti` so an unremapped IRQ0 cannot alias the #DF vector.
        let apic_timer_armed = match unsafe { apic_timer_irq_v25::arm_one_shot_after_idt() } {
            Ok(()) => {
                unsafe {
                    legacy_pic::remap_and_mask_all();
                    apic_timer_irq_v25::enable_interrupts_for_timer_test();
                }
                debug_write("AW_APIC_TIMER_ARMED\n");
                true
            }
            Err(reason) => {
                debug_write("AW_APIC_TIMER_UNAVAILABLE reason=");
                debug_write(reason);
                debug_write("\n");
                false
            }
        };

        if apic_timer_armed {
            let mut spins: u32 = 0;
            while !apic_timer_irq_v25::timer_fired() && spins < 50_000_000 {
                spins += 1;
                core::hint::spin_loop();
            }
            if apic_timer_irq_v25::timer_fired() {
                debug_write("AW_APIC_TIMER_FIRED\n");
            } else {
                debug_write("AW_APIC_TIMER_NOT_FIRED\n");
            }
        }

        // The one-shot timer proof is complete. Re-mask interrupts so the rest
        // of bootstrap and the idle HLT loop keep the interrupts-disabled
        // invariant they were written against: no general IRQ servicing exists
        // yet, and every non-exception vector is a not-present gate.
        // SAFETY: CPL0; simply clears IF.
        unsafe { asm!("cli", options(nomem, nostack, preserves_flags)) };

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

mod interrupt_vectors_v20;

mod local_apic_v20;
