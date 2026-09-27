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
