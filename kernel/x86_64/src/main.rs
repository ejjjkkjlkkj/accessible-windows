#![no_main]
#![no_std]

use aw_kernel_core::{
    HANDOFF_FLAG_FRAMEBUFFER_PRESENT, HANDOFF_FLAG_PCIE_ECAM_PRESENT, HandoffPixelFormat,
    KernelHandoff, PciEcamHandoff,
};
use aw_pci::{PciAddress, PciDeviceIdentity};
use aw_x86_platform::{CpuFeatures, CpuIdentity, CpuSignature, CpuVendor, CpuidRegisters};
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
    // SAFETY: CPUID is available in x86-64 long mode. The intrinsic only reads
    // architectural CPU identification registers and does not dereference memory.
    let registers = unsafe { __cpuid_count(leaf, subleaf) };
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

    CpuIdentity {
        vendor: CpuVendor::from_leaf0(leaf0),
        signature: CpuSignature::from_leaf1_eax(leaf1.eax),
        max_basic_leaf,
        max_extended_leaf,
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

                let header_register =
                    pcie_ecam_read_u32(region, bus, device, 0, 0x0c).unwrap_or(0);
                let header_type = ((header_register >> 16) & 0xff) as u8;
                let function_count = if header_type & 0x80 != 0 { 8 } else { 1 };

                for function in 0_u8..function_count {
                    let vendor_device = pcie_ecam_read_u32(region, bus, device, function, 0x00)
                        .unwrap_or(u32::MAX);
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
                let identity =
                    PciDeviceIdentity::from_config_registers(vendor_device, class_revision, subsystem);
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

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text._start")]
pub extern "sysv64" fn _start(handoff_ptr: *const KernelHandoff) -> ! {
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

    debug_write("AW_NATIVE_KERNEL_ENTRY_OK\n");
    validate_cpu_baseline();
    scan_pci(handoff);

    if paint_boot_marker(handoff) {
        debug_write("AW_NATIVE_FRAMEBUFFER_WRITE_OK\n");
    } else {
        debug_write("AW_NATIVE_FRAMEBUFFER_WRITE_SKIP\n");
    }

    debug_write("AW_NATIVE_KERNEL_IDLE\n");
    halt_forever();
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    debug_write("AW_NATIVE_KERNEL_PANIC\n");
    halt_forever();
}
