#![no_std]
#![forbid(unsafe_code)]

use aw_generation::ObjectId;

pub const MAX_TRIAL_BOOT_ATTEMPTS: u8 = 7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationLocator {
    generation: u64,
    manifest: ObjectId,
}

impl GenerationLocator {
    #[must_use]
    pub const fn new(generation: u64, manifest: ObjectId) -> Option<Self> {
        if generation == 0 {
            return None;
        }
        Some(Self {
            generation,
            manifest,
        })
    }

    #[must_use]
    pub const fn generation(self) -> u64 {
        self.generation
    }

    #[must_use]
    pub const fn manifest(self) -> ObjectId {
        self.manifest
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootSelectionState {
    Trial { tries_remaining: u8 },
    Successful,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootStateRecord {
    sequence: u64,
    selected: GenerationLocator,
    previous_successful: GenerationLocator,
    rollback_floor: u64,
    state: BootSelectionState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootStateError {
    ZeroSequence,
    ZeroRollbackFloor,
    InvalidTrialAttempts,
    NoUsableRecord,
    ConflictingSequence,
    SequenceOverflow,
    NotTrial,
}

impl BootStateRecord {
    pub fn new(
        sequence: u64,
        selected: GenerationLocator,
        previous_successful: GenerationLocator,
        rollback_floor: u64,
        state: BootSelectionState,
    ) -> Result<Self, BootStateError> {
        if sequence == 0 {
            return Err(BootStateError::ZeroSequence);
        }
        if rollback_floor == 0 {
            return Err(BootStateError::ZeroRollbackFloor);
        }
        if let BootSelectionState::Trial { tries_remaining } = state
            && (tries_remaining == 0 || tries_remaining > MAX_TRIAL_BOOT_ATTEMPTS)
        {
            return Err(BootStateError::InvalidTrialAttempts);
        }

        Ok(Self {
            sequence,
            selected,
            previous_successful,
            rollback_floor,
            state,
        })
    }

    #[must_use]
    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    #[must_use]
    pub const fn selected(self) -> GenerationLocator {
        self.selected
    }

    #[must_use]
    pub const fn previous_successful(self) -> GenerationLocator {
        self.previous_successful
    }

    #[must_use]
    pub const fn rollback_floor(self) -> u64 {
        self.rollback_floor
    }

    #[must_use]
    pub const fn state(self) -> BootSelectionState {
        self.state
    }

    /// Returns the next boot-state record after one failed trial boot.
    ///
    /// The record sequence is always advanced. When the final trial attempt is consumed, selection
    /// falls back to the previously successful generation and that generation becomes the active
    /// successful state again.
    pub fn after_failed_trial(self) -> Result<Self, BootStateError> {
        let next_sequence = self
            .sequence
            .checked_add(1)
            .ok_or(BootStateError::SequenceOverflow)?;
        let BootSelectionState::Trial { tries_remaining } = self.state else {
            return Err(BootStateError::NotTrial);
        };

        if tries_remaining > 1 {
            return Ok(Self {
                sequence: next_sequence,
                selected: self.selected,
                previous_successful: self.previous_successful,
                rollback_floor: self.rollback_floor,
                state: BootSelectionState::Trial {
                    tries_remaining: tries_remaining - 1,
                },
            });
        }

        Ok(Self {
            sequence: next_sequence,
            selected: self.previous_successful,
            previous_successful: self.previous_successful,
            rollback_floor: self.rollback_floor,
            state: BootSelectionState::Successful,
        })
    }
}

/// Selects the newest usable copy from two independently validated boot-state records.
///
/// Checksum/authentication validation is intentionally outside this function. Callers must pass
/// only records that already passed their on-disk integrity/authentication checks. Equal sequence
/// numbers with different contents are treated as an ambiguous split-brain condition and fail
/// closed.
pub fn select_newest_record(
    first: Option<BootStateRecord>,
    second: Option<BootStateRecord>,
) -> Result<BootStateRecord, BootStateError> {
    match (first, second) {
        (None, None) => Err(BootStateError::NoUsableRecord),
        (Some(record), None) | (None, Some(record)) => Ok(record),
        (Some(first), Some(second)) => {
            if first.sequence() > second.sequence() {
                Ok(first)
            } else if second.sequence() > first.sequence() {
                Ok(second)
            } else if first == second {
                Ok(first)
            } else {
                Err(BootStateError::ConflictingSequence)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(seed: u8) -> ObjectId {
        ObjectId::new([seed; 32]).unwrap()
    }

    fn locator(generation: u64, seed: u8) -> GenerationLocator {
        GenerationLocator::new(generation, object(seed)).unwrap()
    }

    fn trial(sequence: u64, tries_remaining: u8) -> BootStateRecord {
        BootStateRecord::new(
            sequence,
            locator(42, 1),
            locator(41, 2),
            7,
            BootSelectionState::Trial { tries_remaining },
        )
        .unwrap()
    }

    #[test]
    fn rejects_invalid_record_shape() {
        assert!(matches!(
            BootStateRecord::new(
                0,
                locator(42, 1),
                locator(41, 2),
                7,
                BootSelectionState::Successful
            ),
            Err(BootStateError::ZeroSequence)
        ));
        assert!(matches!(
            BootStateRecord::new(
                1,
                locator(42, 1),
                locator(41, 2),
                0,
                BootSelectionState::Successful
            ),
            Err(BootStateError::ZeroRollbackFloor)
        ));
        assert!(matches!(
            BootStateRecord::new(
                1,
                locator(42, 1),
                locator(41, 2),
                7,
                BootSelectionState::Trial { tries_remaining: 0 }
            ),
            Err(BootStateError::InvalidTrialAttempts)
        ));
        assert!(GenerationLocator::new(0, object(1)).is_none());
    }

    #[test]
    fn redundant_selection_uses_highest_sequence() {
        let selected = select_newest_record(Some(trial(10, 3)), Some(trial(11, 2))).unwrap();
        assert_eq!(selected.sequence(), 11);
        assert_eq!(
            selected.state(),
            BootSelectionState::Trial { tries_remaining: 2 }
        );
    }

    #[test]
    fn equal_sequence_conflict_fails_closed() {
        assert!(matches!(
            select_newest_record(Some(trial(10, 3)), Some(trial(10, 2))),
            Err(BootStateError::ConflictingSequence)
        ));
    }

    #[test]
    fn failed_trial_decrements_attempt_count() {
        let next = trial(10, 3).after_failed_trial().unwrap();
        assert_eq!(next.sequence(), 11);
        assert_eq!(next.selected().generation(), 42);
        assert_eq!(
            next.state(),
            BootSelectionState::Trial { tries_remaining: 2 }
        );
    }

    #[test]
    fn exhausted_trial_rolls_back_to_last_successful_generation() {
        let next = trial(10, 1).after_failed_trial().unwrap();
        assert_eq!(next.sequence(), 11);
        assert_eq!(next.selected().generation(), 41);
        assert_eq!(next.previous_successful().generation(), 41);
        assert_eq!(next.state(), BootSelectionState::Successful);
    }

    #[test]
    fn successful_record_cannot_consume_trial_attempt() {
        let record = BootStateRecord::new(
            10,
            locator(41, 2),
            locator(41, 2),
            7,
            BootSelectionState::Successful,
        )
        .unwrap();
        assert!(matches!(
            record.after_failed_trial(),
            Err(BootStateError::NotTrial)
        ));
    }
}
