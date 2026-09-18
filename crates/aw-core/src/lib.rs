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
        HumanIoContract::new(REQUIRED_ALL, REQUIRED_ANY_OUTPUT, self.verified_human_io)
    }

    /// Produces a boot handoff from this system state.
    #[must_use]
    pub const fn boot_info(self) -> BootInfo {
        BootInfo::new(self.phase, self.human_io_contract())
    }
}

/// Native runtime identity for a semantic object.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticObjectId(u64);

impl SemanticObjectId {
    /// Reserved invalid identity.
    pub const ZERO: Self = Self(0);

    /// Creates an identity.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the underlying value.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    /// Returns true for the reserved invalid identity.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }
}

/// Intrinsic semantic role of a runtime object.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticRole {
    /// System semantic root.
    Root = 1,
    /// Structural region or group.
    Region = 2,
    /// Native textual meaning/content.
    Text = 3,
    /// Invokable action.
    Action = 4,
    /// Value input/editable object.
    Input = 5,
    /// Binary or multi-state toggle.
    Toggle = 6,
    /// Selectable choice.
    Choice = 7,
    /// Collection of semantic objects.
    Collection = 8,
    /// Item inside a collection.
    Item = 9,
    /// Document-like semantic object.
    Document = 10,
    /// Media object.
    Media = 11,
    /// Spatial object/region.
    Spatial = 12,
}

impl SemanticRole {
    /// Returns true when this role represents direct user interaction.
    #[must_use]
    pub const fn is_interactive(self) -> bool {
        matches!(
            self,
            Self::Action | Self::Input | Self::Toggle | Self::Choice
        )
    }
}

/// Native semantic state bits.
pub mod semantic_state {
    /// Object can currently be interacted with.
    pub const ENABLED: u64 = 1 << 0;
    /// Object owns semantic focus.
    pub const FOCUSED: u64 = 1 << 1;
    /// Object is selected.
    pub const SELECTED: u64 = 1 << 2;
    /// Object is expanded.
    pub const EXPANDED: u64 = 1 << 3;
    /// Object is busy updating.
    pub const BUSY: u64 = 1 << 4;
    /// Value is read-only.
    pub const READ_ONLY: u64 = 1 << 5;
    /// Input/value is required.
    pub const REQUIRED: u64 = 1 << 6;
    /// Content is sensitive and requires protected presentation.
    pub const SENSITIVE: u64 = 1 << 7;
}

/// Native semantic action bits.
pub mod semantic_action {
    /// Move semantic focus to the object.
    pub const FOCUS: u64 = 1 << 0;
    /// Invoke/activate the object.
    pub const INVOKE: u64 = 1 << 1;
    /// Set the object's value.
    pub const SET_VALUE: u64 = 1 << 2;
    /// Toggle state.
    pub const TOGGLE: u64 = 1 << 3;
    /// Select the object.
    pub const SELECT: u64 = 1 << 4;
    /// Expand the object.
    pub const EXPAND: u64 = 1 << 5;
    /// Collapse the object.
    pub const COLLAPSE: u64 = 1 << 6;
    /// Perform semantic movement/navigation.
    pub const MOVE: u64 = 1 << 7;
}

/// Validation errors for the native semantic source of truth.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticObjectError {
    /// Every semantic object requires a nonzero identity.
    MissingId,
    /// Interactive objects require native meaning.
    MissingMeaning,
    /// Interactive objects require at least one action.
    MissingAction,
}

/// Native semantic object.
///
/// This object is the system meaning itself. It is not derived from a visual
/// widget and is not an accessibility mirror. Visual, speech, braille, haptic
/// and agent projections will consume this same state.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticObjectV1 {
    /// Stable runtime identity.
    pub id: SemanticObjectId,
    /// Intrinsic role.
    pub role: SemanticRole,
    /// Native state bits.
    pub states: u64,
    /// Native action bits.
    pub actions: u64,
    /// Object carrying the primary human-readable meaning/label. Interactive
    /// objects must reference one; non-interactive content may be self-describing.
    pub meaning: SemanticObjectId,
}

impl SemanticObjectV1 {
    /// Creates a native semantic object.
    #[must_use]
    pub const fn new(
        id: SemanticObjectId,
        role: SemanticRole,
        states: u64,
        actions: u64,
        meaning: SemanticObjectId,
    ) -> Self {
        Self {
            id,
            role,
            states,
            actions,
            meaning,
        }
    }

    /// Validates core semantic invariants.
    pub const fn validate(&self) -> Result<(), SemanticObjectError> {
        if self.id.is_zero() {
            return Err(SemanticObjectError::MissingId);
        }
        if self.role.is_interactive() && self.meaning.is_zero() {
            return Err(SemanticObjectError::MissingMeaning);
        }
        if self.role.is_interactive() && self.actions == 0 {
            return Err(SemanticObjectError::MissingAction);
        }
        Ok(())
    }

    /// Returns true when the object offers an action.
    #[must_use]
    pub const fn supports(self, action: u64) -> bool {
        action != 0 && (self.actions & action) == action
    }
}

/// Native semantic graph relation.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticEdgeKind {
    /// Structural containment.
    Contains = 1,
    /// Source labels/names target.
    Labels = 2,
    /// Source describes target.
    Describes = 3,
    /// Source controls target.
    Controls = 4,
    /// Source owns target.
    Owns = 5,
    /// Interaction proceeds from source to target.
    FlowsTo = 6,
}

/// Validation errors for a semantic graph edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticEdgeError {
    /// Source and target identities must exist.
    MissingEndpoint,
}

/// Native graph edge between semantic objects.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticEdgeV1 {
    /// Source object.
    pub source: SemanticObjectId,
    /// Target object.
    pub target: SemanticObjectId,
    /// Native relation meaning.
    pub kind: SemanticEdgeKind,
}

impl SemanticEdgeV1 {
    /// Creates a semantic edge.
    #[must_use]
    pub const fn new(
        source: SemanticObjectId,
        target: SemanticObjectId,
        kind: SemanticEdgeKind,
    ) -> Self {
        Self {
            source,
            target,
            kind,
        }
    }

    /// Validates edge endpoints.
    pub const fn validate(&self) -> Result<(), SemanticEdgeError> {
        if self.source.is_zero() || self.target.is_zero() {
            return Err(SemanticEdgeError::MissingEndpoint);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_interactive_object_requires_meaning_and_action() {
        let invalid = SemanticObjectV1::new(
            SemanticObjectId::new(10),
            SemanticRole::Action,
            semantic_state::ENABLED,
            0,
            SemanticObjectId::ZERO,
        );
        assert_eq!(invalid.validate(), Err(SemanticObjectError::MissingMeaning));

        let meaning_only = SemanticObjectV1::new(
            SemanticObjectId::new(10),
            SemanticRole::Action,
            semantic_state::ENABLED,
            0,
            SemanticObjectId::new(11),
        );
        assert_eq!(
            meaning_only.validate(),
            Err(SemanticObjectError::MissingAction)
        );
    }

    #[test]
    fn semantic_object_is_modality_independent_source_of_truth() {
        let object = SemanticObjectV1::new(
            SemanticObjectId::new(10),
            SemanticRole::Action,
            semantic_state::ENABLED,
            semantic_action::FOCUS | semantic_action::INVOKE,
            SemanticObjectId::new(11),
        );

        assert_eq!(object.validate(), Ok(()));
        assert!(object.supports(semantic_action::FOCUS));
        assert!(object.supports(semantic_action::INVOKE));
    }

    #[test]
    fn semantic_graph_edge_requires_real_endpoints() {
        let edge = SemanticEdgeV1::new(
            SemanticObjectId::new(10),
            SemanticObjectId::ZERO,
            SemanticEdgeKind::Labels,
        );
        assert_eq!(edge.validate(), Err(SemanticEdgeError::MissingEndpoint));
    }

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
            human_io::KEYBOARD_INPUT | human_io::SEMANTIC_INTERACTION | human_io::BRAILLE_OUTPUT,
        );
        assert!(state.human_io_contract().is_satisfied());
        state.advance_to(BootPhase::Bootloader);
        assert_eq!(state.phase(), BootPhase::Bootloader);
    }
}
