#![no_std]
#![forbid(unsafe_code)]

pub const KERNEL_HANDOFF_MAGIC: u64 = 0x4157_4b48_4f46_4631;
pub const KERNEL_HANDOFF_ABI_VERSION: u32 = 2;
pub const HANDOFF_FLAG_FRAMEBUFFER_PRESENT: u64 = 1 << 0;
pub const HANDOFF_FLAG_PCIE_ECAM_PRESENT: u64 = 1 << 1;
pub const MAX_PCIE_ECAM_REGIONS: usize = 4;

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandoffPixelFormat {
    Unknown = 0,
    Rgb = 1,
    Bgr = 2,
    Bitmask = 3,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FramebufferHandoff {
    pub physical_address: u64,
    pub byte_len: u64,
    pub width: u32,
    pub height: u32,
    pub stride_pixels: u32,
    pub pixel_format: HandoffPixelFormat,
}

impl FramebufferHandoff {
    pub const NONE: Self = Self {
        physical_address: 0,
        byte_len: 0,
        width: 0,
        height: 0,
        stride_pixels: 0,
        pixel_format: HandoffPixelFormat::Unknown,
    };

    #[must_use]
    pub const fn dimensions_are_valid(self) -> bool {
        self.physical_address != 0
            && self.byte_len != 0
            && self.width != 0
            && self.height != 0
            && self.stride_pixels >= self.width
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciEcamHandoff {
    pub base_address: u64,
    pub segment_group: u16,
    pub start_bus: u8,
    pub end_bus: u8,
    pub reserved: u32,
}

impl PciEcamHandoff {
    pub const NONE: Self = Self {
        base_address: 0,
        segment_group: 0,
        start_bus: 0,
        end_bus: 0,
        reserved: 0,
    };

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.base_address != 0
            && self.base_address & 0x000f_ffff == 0
            && self.start_bus <= self.end_bus
            && self.reserved == 0
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelHandoff {
    pub magic: u64,
    pub abi_version: u32,
    pub struct_size: u32,
    pub flags: u64,
    pub acpi_rsdp: u64,
    pub memory_map_entries: u64,
    pub framebuffer: FramebufferHandoff,
    pub pcie_ecam_count: u32,
    pub reserved: u32,
    pub pcie_ecam: [PciEcamHandoff; MAX_PCIE_ECAM_REGIONS],
}

impl KernelHandoff {
    #[must_use]
    pub const fn new(
        acpi_rsdp: u64,
        memory_map_entries: u64,
        framebuffer: Option<FramebufferHandoff>,
        pcie_ecam: [PciEcamHandoff; MAX_PCIE_ECAM_REGIONS],
        pcie_ecam_count: u32,
    ) -> Self {
        let framebuffer_present = framebuffer.is_some();
        let ecam_present = pcie_ecam_count != 0;
        let mut flags = 0;
        if framebuffer_present {
            flags |= HANDOFF_FLAG_FRAMEBUFFER_PRESENT;
        }
        if ecam_present {
            flags |= HANDOFF_FLAG_PCIE_ECAM_PRESENT;
        }

        Self {
            magic: KERNEL_HANDOFF_MAGIC,
            abi_version: KERNEL_HANDOFF_ABI_VERSION,
            struct_size: core::mem::size_of::<Self>() as u32,
            flags,
            acpi_rsdp,
            memory_map_entries,
            framebuffer: match framebuffer {
                Some(framebuffer) => framebuffer,
                None => FramebufferHandoff::NONE,
            },
            pcie_ecam_count,
            reserved: 0,
            pcie_ecam,
        }
    }

    pub fn validate(&self) -> Result<(), HandoffError> {
        if self.magic != KERNEL_HANDOFF_MAGIC {
            return Err(HandoffError::InvalidMagic);
        }
        if self.abi_version != KERNEL_HANDOFF_ABI_VERSION {
            return Err(HandoffError::UnsupportedAbiVersion);
        }
        if self.struct_size < core::mem::size_of::<Self>() as u32 {
            return Err(HandoffError::InvalidStructSize);
        }
        if self.reserved != 0 {
            return Err(HandoffError::InvalidReservedField);
        }
        if self.acpi_rsdp == 0 {
            return Err(HandoffError::MissingAcpiRsdp);
        }
        if self.memory_map_entries == 0 {
            return Err(HandoffError::EmptyMemoryMap);
        }

        let framebuffer_present = self.flags & HANDOFF_FLAG_FRAMEBUFFER_PRESENT != 0;
        if framebuffer_present && !self.framebuffer.dimensions_are_valid() {
            return Err(HandoffError::InvalidFramebuffer);
        }
        if !framebuffer_present && self.framebuffer != FramebufferHandoff::NONE {
            return Err(HandoffError::UnexpectedFramebuffer);
        }

        let ecam_present = self.flags & HANDOFF_FLAG_PCIE_ECAM_PRESENT != 0;
        let ecam_count = self.pcie_ecam_count as usize;
        if ecam_count > MAX_PCIE_ECAM_REGIONS {
            return Err(HandoffError::TooManyPcieEcamRegions);
        }
        if ecam_present != (ecam_count != 0) {
            return Err(HandoffError::InconsistentPcieEcamFlag);
        }

        let mut index = 0;
        while index < MAX_PCIE_ECAM_REGIONS {
            let region = self.pcie_ecam[index];
            if index < ecam_count {
                if !region.is_valid() {
                    return Err(HandoffError::InvalidPcieEcamRegion);
                }
            } else if region != PciEcamHandoff::NONE {
                return Err(HandoffError::UnexpectedPcieEcamRegion);
            }
            index += 1;
        }

        Ok(())
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandoffError {
    InvalidMagic = 1,
    UnsupportedAbiVersion = 2,
    InvalidStructSize = 3,
    MissingAcpiRsdp = 4,
    EmptyMemoryMap = 5,
    InvalidFramebuffer = 6,
    UnexpectedFramebuffer = 7,
    InvalidReservedField = 8,
    TooManyPcieEcamRegions = 9,
    InconsistentPcieEcamFlag = 10,
    InvalidPcieEcamRegion = 11,
    UnexpectedPcieEcamRegion = 12,
}

pub fn enter(handoff: &KernelHandoff) -> Result<(), HandoffError> {
    handoff.validate()
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_FRAMEBUFFER: FramebufferHandoff = FramebufferHandoff {
        physical_address: 0x8000_0000,
        byte_len: 1280 * 800 * 4,
        width: 1280,
        height: 800,
        stride_pixels: 1280,
        pixel_format: HandoffPixelFormat::Bgr,
    };

    const VALID_ECAM: PciEcamHandoff = PciEcamHandoff {
        base_address: 0xe000_0000,
        segment_group: 0,
        start_bus: 0,
        end_bus: 0xff,
        reserved: 0,
    };

    const fn empty_ecam() -> [PciEcamHandoff; MAX_PCIE_ECAM_REGIONS] {
        [PciEcamHandoff::NONE; MAX_PCIE_ECAM_REGIONS]
    }

    fn valid_handoff() -> KernelHandoff {
        let mut ecam = empty_ecam();
        ecam[0] = VALID_ECAM;
        KernelHandoff::new(0xf000_0000, 127, Some(VALID_FRAMEBUFFER), ecam, 1)
    }

    #[test]
    fn accepts_valid_handoff() {
        assert_eq!(enter(&valid_handoff()), Ok(()));
    }

    #[test]
    fn accepts_handoff_without_optional_devices() {
        let handoff = KernelHandoff::new(0xf000_0000, 127, None, empty_ecam(), 0);
        assert_eq!(enter(&handoff), Ok(()));
    }

    #[test]
    fn rejects_wrong_magic() {
        let mut handoff = valid_handoff();
        handoff.magic ^= 1;
        assert_eq!(enter(&handoff), Err(HandoffError::InvalidMagic));
    }

    #[test]
    fn rejects_empty_memory_map() {
        let handoff = KernelHandoff::new(0xf000_0000, 0, None, empty_ecam(), 0);
        assert_eq!(enter(&handoff), Err(HandoffError::EmptyMemoryMap));
    }

    #[test]
    fn rejects_invalid_framebuffer() {
        let mut framebuffer = VALID_FRAMEBUFFER;
        framebuffer.stride_pixels = framebuffer.width - 1;
        let handoff = KernelHandoff::new(
            0xf000_0000,
            127,
            Some(framebuffer),
            empty_ecam(),
            0,
        );
        assert_eq!(enter(&handoff), Err(HandoffError::InvalidFramebuffer));
    }

    #[test]
    fn rejects_inconsistent_ecam_flag() {
        let mut handoff = valid_handoff();
        handoff.flags &= !HANDOFF_FLAG_PCIE_ECAM_PRESENT;
        assert_eq!(
            enter(&handoff),
            Err(HandoffError::InconsistentPcieEcamFlag)
        );
    }

    #[test]
    fn rejects_nonzero_unused_ecam_slot() {
        let mut handoff = valid_handoff();
        handoff.pcie_ecam[1] = VALID_ECAM;
        assert_eq!(
            enter(&handoff),
            Err(HandoffError::UnexpectedPcieEcamRegion)
        );
    }
}
