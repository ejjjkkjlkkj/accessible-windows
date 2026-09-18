#![no_std]
//! Earliest kernel-side validation and bring-up policy.

use aw_abi::BootInfo;

/// Reason the kernel refused an invalid boot transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BringUpError {
    /// The boot handoff does not match ABI v1.
    InvalidBootInfo,
    /// Mandatory intrinsic human-I/O invariants were not present.
    HumanIoContractUnsatisfied {
        /// Mandatory channels that were absent.
        missing_all: u64,
        /// True when no member of the required-any output group was present.
        missing_any: bool,
    },
}

/// Minimal kernel foundation state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KernelFoundation {
    boot_contract_verified: bool,
}

impl KernelFoundation {
    /// Creates an uninitialized kernel foundation.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            boot_contract_verified: false,
        }
    }

    /// Validates the boot ABI and intrinsic human-I/O invariants.
    pub fn accept_boot_info(&mut self, boot_info: &BootInfo) -> Result<(), BringUpError> {
        if !boot_info.is_valid_v1() {
            return Err(BringUpError::InvalidBootInfo);
        }

        let missing_all = boot_info.human_io.missing_all();
        let missing_any = boot_info.human_io.missing_any();
        if missing_all != 0 || missing_any {
            return Err(BringUpError::HumanIoContractUnsatisfied {
                missing_all,
                missing_any,
            });
        }

        self.boot_contract_verified = true;
        Ok(())
    }

    /// Returns true after the transition contract has been verified.
    #[must_use]
    pub const fn boot_contract_verified(self) -> bool {
        self.boot_contract_verified
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aw_abi::{BootInfo, BootPhase, HumanIoContract, human_io};

    #[test]
    fn refuses_visual_only_boot() {
        let info = BootInfo::new(
            BootPhase::Bootloader,
            HumanIoContract::new(
                human_io::KEYBOARD_INPUT | human_io::SEMANTIC_INTERACTION,
                human_io::SPEECH_OUTPUT | human_io::BRAILLE_OUTPUT,
                human_io::KEYBOARD_INPUT | human_io::SEMANTIC_INTERACTION | human_io::VISUAL_OUTPUT,
            ),
        );

        let mut kernel = KernelFoundation::new();
        assert_eq!(
            kernel.accept_boot_info(&info),
            Err(BringUpError::HumanIoContractUnsatisfied {
                missing_all: 0,
                missing_any: true,
            })
        );
    }

    #[test]
    fn accepts_native_non_visual_transition() {
        let required_all = human_io::KEYBOARD_INPUT | human_io::SEMANTIC_INTERACTION;
        let required_any = human_io::SPEECH_OUTPUT | human_io::BRAILLE_OUTPUT;
        let info = BootInfo::new(
            BootPhase::Bootloader,
            HumanIoContract::new(
                required_all,
                required_any,
                required_all | human_io::SPEECH_OUTPUT,
            ),
        );

        let mut kernel = KernelFoundation::new();
        assert_eq!(kernel.accept_boot_info(&info), Ok(()));
        assert!(kernel.boot_contract_verified());
    }
}
