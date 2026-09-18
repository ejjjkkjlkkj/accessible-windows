#![no_std]
//! Accessibility state machine used before user space exists.

use aw_abi::{AccessibilityContract, BootPhase, capability};

/// Strict requirements chosen for the first x86-64 boot path.
pub const STRICT_BOOT_REQUIREMENTS: u64 = capability::KEYBOARD_INPUT | capability::SPEECH_OUTPUT;

/// Accessibility state carried across system phases.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccessibilityState {
    phase: BootPhase,
    available: u64,
}

impl AccessibilityState {
    /// Starts with no capabilities assumed to exist.
    #[must_use]
    pub const fn new(phase: BootPhase) -> Self {
        Self {
            phase,
            available: 0,
        }
    }

    /// Returns the phase represented by this state.
    #[must_use]
    pub const fn phase(self) -> BootPhase {
        self.phase
    }

    /// Returns the observed capability mask.
    #[must_use]
    pub const fn available(self) -> u64 {
        self.available
    }

    /// Records capabilities that were positively verified.
    pub const fn observe(&mut self, capabilities: u64) {
        self.available |= capabilities;
    }

    /// Advances the phase without inventing capabilities.
    pub const fn advance_to(&mut self, phase: BootPhase) {
        self.phase = phase;
    }

    /// Produces the strict boot accessibility contract.
    #[must_use]
    pub const fn strict_contract(self) -> AccessibilityContract {
        AccessibilityContract::new(STRICT_BOOT_REQUIREMENTS, self.available)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_are_never_assumed() {
        let state = AccessibilityState::new(BootPhase::Firmware);
        assert_eq!(state.available(), 0);
        assert_eq!(state.strict_contract().missing(), STRICT_BOOT_REQUIREMENTS);
    }

    #[test]
    fn observed_capabilities_survive_phase_change() {
        let mut state = AccessibilityState::new(BootPhase::Firmware);
        state.observe(capability::KEYBOARD_INPUT | capability::SPEECH_OUTPUT);
        state.advance_to(BootPhase::Bootloader);
        assert_eq!(state.phase(), BootPhase::Bootloader);
        assert!(state.strict_contract().is_satisfied());
    }
}
