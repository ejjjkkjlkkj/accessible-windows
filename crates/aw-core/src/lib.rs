#![no_std]
//! Intrinsic system state shared across the earliest privileged phases.

use aw_abi::{BootInfo, BootPhase, HumanIoContract, human_io};

/// Channels every foundational transition must provide.
pub const REQUIRED_ALL: u64 = human_io::KEYBOARD_INPUT | human_io::SEMANTIC_INTERACTION;
/// At least one native non-visual output channel must be verified.
pub const REQUIRED_ANY_OUTPUT: u64 = human_io::SPEECH_OUTPUT | human_io::BRAILLE_OUTPUT;

/// Earliest unified system state.
///
/// Human interaction is carried here as ordinary system state rather than by a
/// separate accessibility service or framework.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SystemState {
    phase: BootPhase,
    verified_human_io: u64,
}

impl SystemState {
    /// Starts with no hardware or interaction channel assumed to exist.
    #[must_use]
    pub const fn new(phase: BootPhase) -> Self {
        Self {
            phase,
            verified_human_io: 0,
        }
    }

    /// Returns the current privileged phase.
    #[must_use]
    pub const fn phase(self) -> BootPhase {
        self.phase
    }

    /// Returns channels that native code positively verified.
    #[must_use]
    pub const fn verified_human_io(self) -> u64 {
        self.verified_human_io
    }

    /// Records channels after native runtime verification.
    pub const fn verify_human_io(&mut self, channels: u64) {
        self.verified_human_io |= channels;
    }

    /// Advances the phase without inventing capabilities.
    pub const fn advance_to(&mut self, phase: BootPhase) {
        self.phase = phase;
    }

    /// Produces the foundational human-I/O contract.
    #[must_use]
    pub const fn human_io_contract(self) -> HumanIoContract {
        HumanIoContract::new(
            REQUIRED_ALL,
            REQUIRED_ANY_OUTPUT,
            self.verified_human_io,
        )
    }

    /// Produces a boot handoff from this system state.
    #[must_use]
    pub const fn boot_info(self) -> BootInfo {
        BootInfo::new(self.phase, self.human_io_contract())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_state_assumes_nothing() {
        let state = SystemState::new(BootPhase::Firmware);
        assert_eq!(state.verified_human_io(), 0);
        assert!(!state.human_io_contract().is_satisfied());
    }

    #[test]
    fn semantic_keyboard_and_braille_satisfy_foundation() {
        let mut state = SystemState::new(BootPhase::Firmware);
        state.verify_human_io(
            human_io::KEYBOARD_INPUT
                | human_io::SEMANTIC_INTERACTION
                | human_io::BRAILLE_OUTPUT,
        );
        assert!(state.human_io_contract().is_satisfied());
        state.advance_to(BootPhase::Bootloader);
        assert_eq!(state.phase(), BootPhase::Bootloader);
    }
}
