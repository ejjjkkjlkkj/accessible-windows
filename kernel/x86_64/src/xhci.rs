//! Kernel-side xHCI PCI discovery.
//!
//! This stage is deliberately non-destructive: it identifies the controller and
//! its BAR but does not reset it or touch MMIO yet. The next stage can therefore
//! build mapping/ownership rules from concrete controller data instead of
//! guessing an address or taking over an unrelated USB controller.

use aw_kernel_core::{HANDOFF_FLAG_PCIE_ECAM_PRESENT, KernelHandoff};

use crate::debug_write;
use crate::debug_write_hex_u64;
use crate::debug_write_u8;
use crate::pci_config::{BAR0, COMMAND_REGISTER, PciFunction};

const PCI_CLASS_SERIAL_BUS: u8 = 0x0c;
const PCI_SUBCLASS_USB: u8 = 0x03;
const PCI_PROGIF_XHCI: u8 = 0x30;
/// Conservative identity-mapped window for xHCI capabilities, operational
/// registers, ports, runtime registers and doorbells. Mapping does not access
/// the whole span; it only makes future controller registers reachable.
pub const MMIO_WINDOW_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub struct Controller {
    pub function: PciFunction,
    pub bar0: u64,
    pub bar0_is_64: bool,
    pub command: u16,
}

fn is_xhci(class_revision: u32) -> bool {
    (class_revision >> 24) as u8 == PCI_CLASS_SERIAL_BUS
        && (class_revision >> 16) as u8 == PCI_SUBCLASS_USB
        && (class_revision >> 8) as u8 == PCI_PROGIF_XHCI
}

fn decode_bar0(function: PciFunction) -> Option<(u64, bool)> {
    let low = function.read_u32(BAR0)?;
    if low & 1 != 0 {
        return None;
    }

    let memory_type = (low >> 1) & 0x3;
    let low_base = u64::from(low & 0xffff_fff0);
    match memory_type {
        0x0 => (low_base != 0).then_some((low_base, false)),
        0x2 => {
            let high = function.read_u32(BAR0 + 4)?;
            let base = low_base | (u64::from(high) << 32);
            (base != 0).then_some((base, true))
        }
        _ => None,
    }
}

/// Find the first xHCI function described by the firmware-provided ECAM map.
///
/// No PCI configuration writes and no xHCI MMIO accesses occur here.
#[must_use]
pub fn find(handoff: &KernelHandoff) -> Option<Controller> {
    if handoff.flags & HANDOFF_FLAG_PCIE_ECAM_PRESENT == 0 {
        return None;
    }

    for region in handoff
        .pcie_ecam
        .iter()
        .take(handoff.pcie_ecam_count as usize)
        .copied()
    {
        for bus in region.start_bus..=region.end_bus {
            for device in 0u8..32 {
                let function0 = PciFunction::new(region, bus, device, 0)?;
                let vendor0 = function0.read_u16(0x00)?;
                if vendor0 == 0xffff {
                    continue;
                }
                let header = function0.read_u8(0x0e).unwrap_or(0);
                let count = if header & 0x80 != 0 { 8 } else { 1 };

                for function_number in 0u8..count {
                    let Some(function) =
                        PciFunction::new(region, bus, device, function_number)
                    else {
                        continue;
                    };
                    let Some(vendor) = function.read_u16(0x00) else {
                        continue;
                    };
                    if vendor == 0xffff {
                        continue;
                    }
                    let Some(class_revision) = function.read_u32(0x08) else {
                        continue;
                    };
                    if !is_xhci(class_revision) {
                        continue;
                    }
                    let Some((bar0, bar0_is_64)) = decode_bar0(function) else {
                        continue;
                    };
                    let command = function.read_u16(COMMAND_REGISTER).unwrap_or(0);
                    return Some(Controller {
                        function,
                        bar0,
                        bar0_is_64,
                        command,
                    });
                }
            }
        }
    }
    None
}

/// Emit auditable xHCI PCI identity without taking controller ownership.
pub fn prove_pci_discovery(handoff: &KernelHandoff) {
    debug_write("AW_XHCI_PCI_BEGIN\n");
    let Some(controller) = find(handoff) else {
        debug_write("AW_XHCI_PCI_UNAVAILABLE\n");
        return;
    };

    debug_write("AW_XHCI_PCI_FOUND bus=");
    debug_write_u8(controller.function.bus());
    debug_write(" device=");
    debug_write_u8(controller.function.device());
    debug_write(" function=");
    debug_write_u8(controller.function.function());
    debug_write(" bar0=");
    debug_write_hex_u64(controller.bar0);
    debug_write(" bar64=");
    debug_write_u8(u8::from(controller.bar0_is_64));
    debug_write(" command=");
    debug_write_hex_u64(u64::from(controller.command));
    debug_write("\n");
    debug_write("AW_XHCI_PCI_DISCOVERY_PROOF_OK\n");
}


const CAP_HCSPARAMS1: u64 = 0x04;
const CAP_HCCPARAMS1: u64 = 0x10;
const CAP_DBOFF: u64 = 0x14;
const CAP_RTSOFF: u64 = 0x18;

unsafe fn mmio_read_u32(address: u64) -> u32 {
    // SAFETY: caller proves the address is inside the active low identity map
    // and belongs to the discovered xHCI BAR.
    unsafe { (address as *const u32).read_volatile() }
}

/// Read and validate xHCI capability registers without changing controller
/// state. This proves that the BAR is not only discoverable through PCI config
/// space but actually reachable through the kernel-owned page tables.
pub fn prove_mmio_capabilities(handoff: &KernelHandoff, memory_ready: bool) {
    debug_write("AW_XHCI_MMIO_BEGIN\n");
    if !memory_ready {
        debug_write("AW_XHCI_MMIO_UNAVAILABLE reason=vmm_not_ready\n");
        return;
    }
    let Some(controller) = find(handoff) else {
        debug_write("AW_XHCI_MMIO_UNAVAILABLE reason=no_controller\n");
        return;
    };

    let Some(last_register) = controller.bar0.checked_add(CAP_RTSOFF + 4) else {
        debug_write("AW_XHCI_MMIO_UNAVAILABLE reason=bar_overflow\n");
        return;
    };
    let Some(mapped_end) = controller.bar0.checked_add(MMIO_WINDOW_BYTES) else {
        debug_write("AW_XHCI_MMIO_UNAVAILABLE reason=window_overflow\n");
        return;
    };
    if last_register > mapped_end {
        debug_write("AW_XHCI_MMIO_UNAVAILABLE reason=window_too_small\n");
        return;
    }

    // SAFETY: activate_virtual_memory adds this controller's MMIO window to the
    // audited identity map before CR3 is switched, and BAR0 came from xHCI PCI.
    let cap0 = unsafe { mmio_read_u32(controller.bar0) };
    let cap_length = (cap0 & 0xff) as u8;
    let hcs1 = unsafe { mmio_read_u32(controller.bar0 + CAP_HCSPARAMS1) };
    let hcc1 = unsafe { mmio_read_u32(controller.bar0 + CAP_HCCPARAMS1) };
    let dboff = unsafe { mmio_read_u32(controller.bar0 + CAP_DBOFF) };
    let rtsoff = unsafe { mmio_read_u32(controller.bar0 + CAP_RTSOFF) };

    let max_slots = (hcs1 & 0xff) as u8;
    let max_ports = ((hcs1 >> 24) & 0xff) as u8;
    let context_bytes = if hcc1 & (1 << 2) != 0 { 64 } else { 32 };

    if cap_length < 0x20
        || max_slots == 0
        || max_ports == 0
        || dboff & 0x3 != 0
        || rtsoff & 0x1f != 0
    {
        debug_write("AW_XHCI_MMIO_FAIL reason=capability_shape\n");
        return;
    }

    debug_write("AW_XHCI_MMIO_CAP caplen=");
    debug_write_u8(cap_length);
    debug_write(" slots=");
    debug_write_u8(max_slots);
    debug_write(" ports=");
    debug_write_u8(max_ports);
    debug_write(" ctx=");
    debug_write_u8(context_bytes);
    debug_write(" dboff=");
    debug_write_hex_u64(u64::from(dboff));
    debug_write(" rtsoff=");
    debug_write_hex_u64(u64::from(rtsoff));
    debug_write("\n");
    debug_write("AW_XHCI_MMIO_CAP_PROOF_OK\n");
}
