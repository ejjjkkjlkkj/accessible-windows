#![no_std]
//! Stable data contracts shared by the boot environment and the kernel.

/// Magic identifying a valid Accessible Windows boot handoff.
pub const BOOT_INFO_MAGIC: u64 = 0x4157_4F53_424F_4F54;
/// First version of the handoff ABI.
pub const BOOT_ABI_VERSION: u32 = 1;

/// Boot stage at which a record was produced.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootPhase {
    /// Firmware is still in control.
    Firmware = 0,
    /// Accessible Windows boot code is executing.
    Bootloader = 1,
    /// Earliest kernel bring-up.
    KernelEarly = 2,
    /// Core kernel services are online.
    KernelServices = 3,
    /// User-space services are online.
    UserSpace = 4,
}

/// Accessibility capabilities carried through every boot transition.
pub mod capability {
    /// Physical keyboard input can be consumed without pointer interaction.
    pub const KEYBOARD_INPUT: u64 = 1 << 0;
    /// Speech output is available.
    pub const SPEECH_OUTPUT: u64 = 1 << 1;
    /// Braille output is available.
    pub const BRAILLE_OUTPUT: u64 = 1 << 2;
    /// Serial text can be emitted as an engineering fallback.
    pub const SERIAL_OUTPUT: u64 = 1 << 3;
    /// Structured semantic UI information is available.
    pub const SEMANTIC_UI: u64 = 1 << 4;
}

/// Accessibility requirements and observations for a boot transition.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccessibilityContract {
    /// Capabilities that must exist before the transition is considered accessible.
    pub required: u64,
    /// Capabilities positively observed at runtime.
    pub available: u64,
}

impl AccessibilityContract {
    /// Creates a new contract.
    #[must_use]
    pub const fn new(required: u64, available: u64) -> Self {
        Self {
            required,
            available,
        }
    }

    /// Returns the required capabilities that are not currently available.
    #[must_use]
    pub const fn missing(self) -> u64 {
        self.required & !self.available
    }

    /// Returns true only when every required capability is available.
    #[must_use]
    pub const fn is_satisfied(self) -> bool {
        self.missing() == 0
    }
}

/// Minimal versioned handoff from boot code to the kernel.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootInfo {
    /// BOOT_INFO_MAGIC.
    pub magic: u64,
    /// ABI version understood by both sides.
    pub abi_version: u32,
    /// Size of this structure, enabling future compatible extension.
    pub struct_size: u32,
    /// Phase that produced this record.
    pub phase: BootPhase,
    /// Reserved for future flags; must be zero in ABI v1.
    pub flags: u32,
    /// Accessibility state that must survive the transition.
    pub accessibility: AccessibilityContract,
}

impl BootInfo {
    /// Creates an ABI-v1 boot record.
    #[must_use]
    pub const fn new(phase: BootPhase, accessibility: AccessibilityContract) -> Self {
        Self {
            magic: BOOT_INFO_MAGIC,
            abi_version: BOOT_ABI_VERSION,
            struct_size: core::mem::size_of::<Self>() as u32,
            phase,
            flags: 0,
            accessibility,
        }
    }

    /// Validates the immutable ABI fields.
    #[must_use]
    pub const fn is_valid_v1(&self) -> bool {
        self.magic == BOOT_INFO_MAGIC
            && self.abi_version == BOOT_ABI_VERSION
            && self.struct_size == core::mem::size_of::<Self>() as u32
            && self.flags == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_missing_accessibility_capabilities() {
        let contract = AccessibilityContract::new(
            capability::KEYBOARD_INPUT | capability::SPEECH_OUTPUT,
            capability::KEYBOARD_INPUT,
        );
        assert_eq!(contract.missing(), capability::SPEECH_OUTPUT);
        assert!(!contract.is_satisfied());
    }

    #[test]
    fn validates_v1_boot_info() {
        let info = BootInfo::new(
            BootPhase::Bootloader,
            AccessibilityContract::new(capability::KEYBOARD_INPUT, capability::KEYBOARD_INPUT),
        );
        assert!(info.is_valid_v1());
    }
}
