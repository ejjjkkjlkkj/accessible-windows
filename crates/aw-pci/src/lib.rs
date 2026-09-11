#![no_std]
#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciAddress {
    pub segment: u16,
    pub bus: u8,
    pub device: u8,
    pub function: u8,
}

impl PciAddress {
    pub const fn new(segment: u16, bus: u8, device: u8, function: u8) -> Option<Self> {
        if device > 31 || function > 7 {
            return None;
        }
        Some(Self {
            segment,
            bus,
            device,
            function,
        })
    }

    pub const fn ecam_offset(self, register_offset: u16) -> Option<u64> {
        if register_offset > 0x0fff {
            return None;
        }
        Some(
            ((self.bus as u64) << 20)
                | ((self.device as u64) << 15)
                | ((self.function as u64) << 12)
                | register_offset as u64,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciClassCode {
    pub base: u8,
    pub subclass: u8,
    pub programming_interface: u8,
}

impl PciClassCode {
    pub const fn new(base: u8, subclass: u8, programming_interface: u8) -> Self {
        Self {
            base,
            subclass,
            programming_interface,
        }
    }

    pub const fn base_class(self) -> PciBaseClass {
        match self.base {
            0x01 => PciBaseClass::MassStorage,
            0x02 => PciBaseClass::Network,
            0x03 => PciBaseClass::Display,
            0x04 => PciBaseClass::Multimedia,
            0x06 => PciBaseClass::Bridge,
            0x0c => PciBaseClass::SerialBus,
            value => PciBaseClass::Other(value),
        }
    }

    pub const fn is_nvme(self) -> bool {
        self.base == 0x01 && self.subclass == 0x08 && self.programming_interface == 0x02
    }

    pub const fn is_ahci(self) -> bool {
        self.base == 0x01 && self.subclass == 0x06 && self.programming_interface == 0x01
    }

    pub const fn is_xhci(self) -> bool {
        self.base == 0x0c && self.subclass == 0x03 && self.programming_interface == 0x30
    }

    pub const fn is_hda(self) -> bool {
        self.base == 0x04 && self.subclass == 0x03
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PciBaseClass {
    MassStorage,
    Network,
    Display,
    Multimedia,
    Bridge,
    SerialBus,
    Other(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciDeviceIdentity {
    pub vendor_id: u16,
    pub device_id: u16,
    pub subsystem_vendor_id: Option<u16>,
    pub subsystem_device_id: Option<u16>,
    pub class: PciClassCode,
}

impl PciDeviceIdentity {
    pub const fn is_present(self) -> bool {
        self.vendor_id != 0xffff
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_device_or_function_numbers() {
        assert!(PciAddress::new(0, 0, 31, 7).is_some());
        assert!(PciAddress::new(0, 0, 32, 0).is_none());
        assert!(PciAddress::new(0, 0, 0, 8).is_none());
    }

    #[test]
    fn computes_standard_pcie_ecam_offsets() {
        let address = PciAddress::new(0, 2, 5, 3).unwrap();
        assert_eq!(
            address.ecam_offset(0x120),
            Some((2_u64 << 20) | (5_u64 << 15) | (3_u64 << 12) | 0x120)
        );
        assert_eq!(address.ecam_offset(0x1000), None);
    }

    #[test]
    fn classifies_boot_critical_standard_controllers() {
        assert!(PciClassCode::new(0x01, 0x08, 0x02).is_nvme());
        assert!(PciClassCode::new(0x01, 0x06, 0x01).is_ahci());
        assert!(PciClassCode::new(0x0c, 0x03, 0x30).is_xhci());
        assert!(PciClassCode::new(0x04, 0x03, 0x00).is_hda());
    }

    #[test]
    fn keeps_unknown_classes_forward_compatible() {
        assert_eq!(
            PciClassCode::new(0xff, 0, 0).base_class(),
            PciBaseClass::Other(0xff)
        );
    }
}
