#![no_std]
//! Earliest kernel-side validation and bring-up policy.

use aw_abi::BootInfo;

/// Reason the kernel refused an invalid boot transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BringUpError {
    /// The boot handoff does not match ABI v1.
    InvalidBootInfo,
    /// Mandatory accessibility capabilities were not present.
    AccessibilityContractUnsatisfied {
        /// Bitmask of required capabilities that were missing.
        missing: u64,
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

    /// Validates the boot ABI and accessibility contract before later initialization.
    pub fn accept_boot_info(&mut self, boot_info: &BootInfo) -> Result<(), BringUpError> {
        if !boot_info.is_valid_v1() {
            return Err(BringUpError::InvalidBootInfo);
        }

        let missing = boot_info.accessibility.missing();
        if missing != 0 {
            return Err(BringUpError::AccessibilityContractUnsatisfied { missing });
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
    use aw_abi::{AccessibilityContract, BootInfo, BootPhase, capability};

    #[test]
    fn refuses_boot_without_required_accessibility() {
        let info = BootInfo::new(
            BootPhase::Bootloader,
            AccessibilityContract::new(
                capability::KEYBOARD_INPUT | capability::SPEECH_OUTPUT,
                capability::KEYBOARD_INPUT,
            ),
        );
        let mut kernel = KernelFoundation::new();
        assert_eq!(
            kernel.accept_boot_info(&info),
            Err(BringUpError::AccessibilityContractUnsatisfied {
                missing: capability::SPEECH_OUTPUT,
            })
        );
    }

    #[test]
    fn accepts_valid_accessible_transition() {
        let required = capability::KEYBOARD_INPUT | capability::SPEECH_OUTPUT;
        let info = BootInfo::new(
            BootPhase::Bootloader,
            AccessibilityContract::new(required, required),
        );
        let mut kernel = KernelFoundation::new();
        assert_eq!(kernel.accept_boot_info(&info), Ok(()));
        assert!(kernel.boot_contract_verified());
    }
}
