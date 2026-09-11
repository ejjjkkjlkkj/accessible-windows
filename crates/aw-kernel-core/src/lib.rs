#![no_std]
#![forbid(unsafe_code)]

pub const KERNEL_HANDOFF_MAGIC: u64 = 0x4157_4b48_4f46_4631;
pub const KERNEL_HANDOFF_ABI_VERSION: u32 = 1;
pub const HANDOFF_FLAG_FRAMEBUFFER_PRESENT: u64 = 1 << 0;

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
pub struct KernelHandoff {
    pub magic: u64,
    pub abi_version: u32,
    pub struct_size: u32,
    pub flags: u64,
    pub acpi_rsdp: u64,
    pub memory_map_entries: u64,
    pub framebuffer: FramebufferHandoff,
}

impl KernelHandoff {
    #[must_use]
    pub const fn new(
        acpi_rsdp: u64,
        memory_map_entries: u64,
        framebuffer: Option<FramebufferHandoff>,
    ) -> Self {
        match framebuffer {
            Some(framebuffer) => Self {
                magic: KERNEL_HANDOFF_MAGIC,
                abi_version: KERNEL_HANDOFF_ABI_VERSION,
                struct_size: core::mem::size_of::<Self>() as u32,
                flags: HANDOFF_FLAG_FRAMEBUFFER_PRESENT,
                acpi_rsdp,
                memory_map_entries,
                framebuffer,
            },
            None => Self {
                magic: KERNEL_HANDOFF_MAGIC,
                abi_version: KERNEL_HANDOFF_ABI_VERSION,
                struct_size: core::mem::size_of::<Self>() as u32,
                flags: 0,
                acpi_rsdp,
                memory_map_entries,
                framebuffer: FramebufferHandoff::NONE,
            },
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

    #[test]
    fn accepts_valid_handoff() {
        let handoff = KernelHandoff::new(0xf000_0000, 127, Some(VALID_FRAMEBUFFER));
        assert_eq!(enter(&handoff), Ok(()));
    }

    #[test]
    fn accepts_handoff_without_framebuffer() {
        let handoff = KernelHandoff::new(0xf000_0000, 127, None);
        assert_eq!(enter(&handoff), Ok(()));
    }

    #[test]
    fn rejects_wrong_magic() {
        let mut handoff = KernelHandoff::new(0xf000_0000, 127, None);
        handoff.magic ^= 1;
        assert_eq!(enter(&handoff), Err(HandoffError::InvalidMagic));
    }

    #[test]
    fn rejects_empty_memory_map() {
        let handoff = KernelHandoff::new(0xf000_0000, 0, None);
        assert_eq!(enter(&handoff), Err(HandoffError::EmptyMemoryMap));
    }

    #[test]
    fn rejects_invalid_framebuffer() {
        let mut framebuffer = VALID_FRAMEBUFFER;
        framebuffer.stride_pixels = framebuffer.width - 1;
        let handoff = KernelHandoff::new(0xf000_0000, 127, Some(framebuffer));
        assert_eq!(enter(&handoff), Err(HandoffError::InvalidFramebuffer));
    }
}
