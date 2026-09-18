#![no_std]
//! Versioned contracts shared by firmware, boot code and the kernel.

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
    /// Native boot code is executing.
    Bootloader = 1,
    /// Earliest kernel bring-up.
    KernelEarly = 2,
    /// Core kernel services are online.
    KernelServices = 3,
    /// User-space services are online.
    UserSpace = 4,
}

/// Native human-I/O channels carried as intrinsic system state.
pub mod human_io {
    /// Physical keyboard input can be consumed without pointer interaction.
    pub const KEYBOARD_INPUT: u64 = 1 << 0;
    /// Native speech output is available.
    pub const SPEECH_OUTPUT: u64 = 1 << 1;
    /// Native braille output is available.
    pub const BRAILLE_OUTPUT: u64 = 1 << 2;
    /// Structured semantic interaction information is available.
    pub const SEMANTIC_INTERACTION: u64 = 1 << 3;
    /// Visual output is available as one system modality.
    pub const VISUAL_OUTPUT: u64 = 1 << 4;
    /// Haptic output is available.
    pub const HAPTIC_OUTPUT: u64 = 1 << 5;
    /// Serial text can be emitted as an engineering diagnostic fallback.
    pub const SERIAL_DIAGNOSTIC: u64 = 1 << 6;
}

/// Intrinsic human-I/O requirements for a privileged transition.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HumanIoContract {
    /// Channels that must all be verified.
    pub required_all: u64,
    /// Set from which at least one channel must be verified.
    pub required_any: u64,
    /// Channels positively verified at runtime.
    pub available: u64,
}

impl HumanIoContract {
    /// Creates a contract without assuming any capability exists.
    #[must_use]
    pub const fn new(required_all: u64, required_any: u64, available: u64) -> Self {
        Self {
            required_all,
            required_any,
            available,
        }
    }

    /// Returns mandatory channels that are missing.
    #[must_use]
    pub const fn missing_all(self) -> u64 {
        self.required_all & !self.available
    }

    /// Returns whether the required-any group is unsatisfied.
    #[must_use]
    pub const fn missing_any(self) -> bool {
        self.required_any != 0 && (self.required_any & self.available) == 0
    }

    /// Returns true only when the complete human-I/O contract is satisfied.
    #[must_use]
    pub const fn is_satisfied(self) -> bool {
        self.missing_all() == 0 && !self.missing_any()
    }
}

/// Minimal versioned handoff from boot code to the kernel.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootInfo {
    /// `BOOT_INFO_MAGIC`.
    pub magic: u64,
    /// ABI version understood by both sides.
    pub abi_version: u32,
    /// Size of this structure, enabling future compatible extension.
    pub struct_size: u32,
    /// Phase that produced this record.
    pub phase: BootPhase,
    /// Reserved for future flags; must be zero in ABI v1.
    pub flags: u32,
    /// Human interaction state that must survive the transition.
    pub human_io: HumanIoContract,
}

impl BootInfo {
    /// Creates an ABI-v1 boot record.
    #[must_use]
    pub const fn new(phase: BootPhase, human_io: HumanIoContract) -> Self {
        Self {
            magic: BOOT_INFO_MAGIC,
            abi_version: BOOT_ABI_VERSION,
            struct_size: core::mem::size_of::<Self>() as u32,
            phase,
            flags: 0,
            human_io,
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
    fn requires_all_and_one_non_visual_output() {
        let contract = HumanIoContract::new(
            human_io::KEYBOARD_INPUT | human_io::SEMANTIC_INTERACTION,
            human_io::SPEECH_OUTPUT | human_io::BRAILLE_OUTPUT,
            human_io::KEYBOARD_INPUT | human_io::SEMANTIC_INTERACTION,
        );
        assert_eq!(contract.missing_all(), 0);
        assert!(contract.missing_any());
        assert!(!contract.is_satisfied());
    }

    #[test]
    fn accepts_speech_or_braille_without_visual_dependency() {
        let required_all = human_io::KEYBOARD_INPUT | human_io::SEMANTIC_INTERACTION;
        let required_any = human_io::SPEECH_OUTPUT | human_io::BRAILLE_OUTPUT;
        let contract = HumanIoContract::new(
            required_all,
            required_any,
            required_all | human_io::BRAILLE_OUTPUT,
        );
        assert!(contract.is_satisfied());
    }
}
