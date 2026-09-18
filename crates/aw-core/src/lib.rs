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

/// Native identity for a cognitive-memory record.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct MemoryRecordId(u64);

impl MemoryRecordId {
    /// Reserved invalid identity.
    pub const ZERO: Self = Self(0);

    /// Creates a memory-record identity.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns true for the reserved invalid identity.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }
}

/// Native memory class.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryClass {
    /// Short-lived active/working context.
    Working = 1,
    /// Event/history memory.
    Episodic = 2,
    /// Meaning and knowledge memory.
    Semantic = 3,
    /// Learned procedure or skill memory.
    Procedural = 4,
}

/// Retention intent for native memory.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryRetention {
    /// May disappear as soon as the active task ends.
    Ephemeral = 1,
    /// Retained for the current system/user session.
    Session = 2,
    /// Durable native memory.
    Durable = 3,
    /// Explicitly retained under user control.
    UserPinned = 4,
}

/// Semantic links carried by a memory record.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MemoryLinksV1 {
    /// Semantic subject this memory is about.
    pub subject: SemanticObjectId,
    /// Semantic content/meaning retained.
    pub content: SemanticObjectId,
    /// Semantic source/provenance describing where the memory came from.
    pub provenance: SemanticObjectId,
}

impl MemoryLinksV1 {
    /// Creates memory links.
    #[must_use]
    pub const fn new(
        subject: SemanticObjectId,
        content: SemanticObjectId,
        provenance: SemanticObjectId,
    ) -> Self {
        Self {
            subject,
            content,
            provenance,
        }
    }
}

/// Validation errors for native cognitive memory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryRecordError {
    /// Every memory record requires a nonzero identity.
    MissingId,
    /// Generation zero is reserved.
    InvalidGeneration,
    /// Subject semantic identity is required.
    MissingSubject,
    /// Content semantic identity is required.
    MissingContent,
    /// Provenance is mandatory; native memory must not silently invent origin.
    MissingProvenance,
    /// Confidence is expressed in parts-per-million and cannot exceed 1,000,000.
    InvalidConfidence,
    /// Reserved version-1 flags must remain zero.
    ReservedFlagsSet,
}

/// Version-1 native cognitive-memory record.
///
/// Memory is versioned and provenance-bearing system state. This is a structural
/// foundation only; it does not implement learning, retrieval or persistence.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryRecordV1 {
    /// Identity of this immutable memory version.
    pub id: MemoryRecordId,
    /// Previous version or `MemoryRecordId::ZERO`.
    pub previous_version: MemoryRecordId,
    /// Monotonic memory generation.
    pub generation: u64,
    /// Cognitive memory class.
    pub class: MemoryClass,
    /// Retention intent.
    pub retention: MemoryRetention,
    /// Semantic subject/content/provenance links.
    pub links: MemoryLinksV1,
    /// Confidence in parts-per-million, from 0 through 1,000,000.
    pub confidence_ppm: u32,
    /// Reserved version-1 flags.
    pub flags: u32,
}

impl MemoryRecordV1 {
    /// Creates a version-1 native memory record.
    #[must_use]
    pub const fn new(
        id: MemoryRecordId,
        previous_version: MemoryRecordId,
        generation: u64,
        class: MemoryClass,
        retention: MemoryRetention,
        links: MemoryLinksV1,
        confidence_ppm: u32,
    ) -> Self {
        Self {
            id,
            previous_version,
            generation,
            class,
            retention,
            links,
            confidence_ppm,
            flags: 0,
        }
    }

    /// Validates native memory invariants.
    pub const fn validate(&self) -> Result<(), MemoryRecordError> {
        if self.id.is_zero() {
            return Err(MemoryRecordError::MissingId);
        }
        if self.generation == 0 {
            return Err(MemoryRecordError::InvalidGeneration);
        }
        if self.links.subject.is_zero() {
            return Err(MemoryRecordError::MissingSubject);
        }
        if self.links.content.is_zero() {
            return Err(MemoryRecordError::MissingContent);
        }
        if self.links.provenance.is_zero() {
            return Err(MemoryRecordError::MissingProvenance);
        }
        if self.confidence_ppm > 1_000_000 {
            return Err(MemoryRecordError::InvalidConfidence);
        }
        if self.flags != 0 {
            return Err(MemoryRecordError::ReservedFlagsSet);
        }
        Ok(())
    }
}

/// Native identity for an explicit capability grant.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct CapabilityId(u64);

impl CapabilityId {
    /// Reserved identity meaning "no parent/no capability".
    pub const ZERO: Self = Self(0);

    /// Creates a capability identity.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns true for the reserved invalid identity.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }
}

/// Native authority domains.
///
/// These identities are intentionally distinct. In particular,
/// `SystemSovereign` is a human authority and is never an alias for
/// `KernelAuthority`.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorityDomain {
    /// Internal non-human kernel authority.
    KernelAuthority = 1,
    /// Highest human system authority.
    SystemSovereign = 2,
    /// Firmware/pre-boot authority.
    FirmwareAuthority = 3,
    /// Recovery environment authority.
    RecoveryAuthority = 4,
    /// Cognitive planner. It may propose but must not mutate system state.
    AgentPlanner = 5,
    /// Capability-scoped agent action executor.
    AgentExecutor = 6,
    /// Ordinary user process domain.
    UserProcess = 7,
}

/// Native capability rights.
pub mod capability_right {
    /// Observe/read the target.
    pub const READ: u64 = 1 << 0;
    /// Modify target state.
    pub const WRITE: u64 = 1 << 1;
    /// Invoke a target action.
    pub const INVOKE: u64 = 1 << 2;
    /// Change target configuration.
    pub const CONFIGURE: u64 = 1 << 3;
    /// Perform recovery operations on the target.
    pub const RECOVER: u64 = 1 << 4;
    /// Update/replace the target through its controlled update path.
    pub const UPDATE: u64 = 1 << 5;
    /// Delegate a subset of held authority.
    pub const DELEGATE: u64 = 1 << 6;

    /// Rights that can mutate or extend authority.
    pub const MUTATING: u64 = WRITE | INVOKE | CONFIGURE | RECOVER | UPDATE | DELEGATE;
}

/// Target and lifetime of a capability.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityScopeV1 {
    /// Semantic resource/action target.
    pub resource: SemanticObjectId,
    /// Explicit rights only; no ambient authority exists in this contract.
    pub rights: u64,
    /// First system generation in which the capability is valid.
    pub valid_from_generation: u64,
    /// Last system generation in which the capability is valid, inclusive.
    pub valid_until_generation: u64,
}

impl CapabilityScopeV1 {
    /// Creates an explicit capability scope.
    #[must_use]
    pub const fn new(
        resource: SemanticObjectId,
        rights: u64,
        valid_from_generation: u64,
        valid_until_generation: u64,
    ) -> Self {
        Self {
            resource,
            rights,
            valid_from_generation,
            valid_until_generation,
        }
    }

    /// Returns true when the scope is active for a generation.
    #[must_use]
    pub const fn is_active_at(self, generation: u64) -> bool {
        generation >= self.valid_from_generation && generation <= self.valid_until_generation
    }
}

/// Validation errors for a native capability grant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityGrantError {
    /// Every grant requires a nonzero identity.
    MissingId,
    /// Every grant is scoped to a concrete semantic target.
    MissingResource,
    /// A capability with no rights conveys nothing and is invalid.
    MissingRights,
    /// Capability lifetime is malformed or starts at generation zero.
    InvalidGenerationWindow,
    /// Planning and privileged execution are separated by construction.
    PlannerMayNotMutate,
    /// Kernel authority cannot be minted by another domain or delegated.
    KernelAuthorityNotDelegable,
}

/// Version-1 explicit capability grant.
///
/// This contract models authority flow only. Cryptographic/unforgeable hardware
/// capability enforcement is not implemented yet and must not be inferred from
/// the existence of this structure.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityGrantV1 {
    /// Stable grant identity.
    pub id: CapabilityId,
    /// Parent grant for explicit delegation, or zero for a root grant.
    pub parent: CapabilityId,
    /// Authority issuing the grant.
    pub issuer: AuthorityDomain,
    /// Authority receiving the grant.
    pub subject: AuthorityDomain,
    /// Exact resource, rights and lifetime.
    pub scope: CapabilityScopeV1,
}

impl CapabilityGrantV1 {
    /// Creates a native capability grant.
    #[must_use]
    pub const fn new(
        id: CapabilityId,
        parent: CapabilityId,
        issuer: AuthorityDomain,
        subject: AuthorityDomain,
        scope: CapabilityScopeV1,
    ) -> Self {
        Self {
            id,
            parent,
            issuer,
            subject,
            scope,
        }
    }

    /// Validates structural authority invariants.
    pub const fn validate(&self) -> Result<(), CapabilityGrantError> {
        if self.id.is_zero() {
            return Err(CapabilityGrantError::MissingId);
        }
        if self.scope.resource.is_zero() {
            return Err(CapabilityGrantError::MissingResource);
        }
        if self.scope.rights == 0 {
            return Err(CapabilityGrantError::MissingRights);
        }
        if self.scope.valid_from_generation == 0
            || self.scope.valid_from_generation > self.scope.valid_until_generation
        {
            return Err(CapabilityGrantError::InvalidGenerationWindow);
        }
        if matches!(self.subject, AuthorityDomain::AgentPlanner)
            && (self.scope.rights & capability_right::MUTATING) != 0
        {
            return Err(CapabilityGrantError::PlannerMayNotMutate);
        }
        if matches!(self.subject, AuthorityDomain::KernelAuthority)
            && (!matches!(self.issuer, AuthorityDomain::KernelAuthority) || !self.parent.is_zero())
        {
            return Err(CapabilityGrantError::KernelAuthorityNotDelegable);
        }
        Ok(())
    }

    /// Returns true only when the grant validates and contains every requested
    /// right for the given resource and system generation.
    #[must_use]
    pub const fn authorizes(
        &self,
        resource: SemanticObjectId,
        requested_rights: u64,
        generation: u64,
    ) -> bool {
        self.validate().is_ok()
            && !resource.is_zero()
            && requested_rights != 0
            && self.scope.resource.value() == resource.value()
            && (self.scope.rights & requested_rights) == requested_rights
            && self.scope.is_active_at(generation)
    }
}

/// Native identity for a cognitive intent.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct CognitiveIntentId(u64);

impl CognitiveIntentId {
    /// Reserved invalid identity.
    pub const ZERO: Self = Self(0);

    /// Creates an intent identity.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns true for the reserved invalid identity.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }
}

/// Validation errors for a native cognitive intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CognitiveIntentError {
    /// Every intent requires an identity.
    MissingId,
    /// Goal semantics are mandatory.
    MissingGoal,
    /// A concrete semantic target is mandatory.
    MissingTarget,
    /// Planning requires explicit memory/evidence provenance.
    MissingEvidence,
    /// An intent must request at least one explicit right.
    MissingRequestedRights,
    /// Generation zero is reserved.
    InvalidGeneration,
    /// Only the planner domain creates cognitive intents.
    PlannerDomainRequired,
}

/// Native cognitive intent.
///
/// An intent is a proposal, never an authority token. It may request a
/// privileged action, but it cannot execute that action by itself.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CognitiveIntentV1 {
    /// Stable intent identity.
    pub id: CognitiveIntentId,
    /// Semantic description of the intended goal.
    pub goal: SemanticObjectId,
    /// Concrete semantic target of the proposed action.
    pub target: SemanticObjectId,
    /// Memory/evidence supporting the proposal.
    pub evidence: MemoryRecordId,
    /// Rights the proposal would require if authorized.
    pub requested_rights: u64,
    /// System generation in which the intent was produced.
    pub generation: u64,
    /// Domain that produced the plan.
    pub planner: AuthorityDomain,
}

impl CognitiveIntentV1 {
    /// Creates a native cognitive intent.
    #[must_use]
    pub const fn new(
        id: CognitiveIntentId,
        goal: SemanticObjectId,
        target: SemanticObjectId,
        evidence: MemoryRecordId,
        requested_rights: u64,
        generation: u64,
        planner: AuthorityDomain,
    ) -> Self {
        Self {
            id,
            goal,
            target,
            evidence,
            requested_rights,
            generation,
            planner,
        }
    }

    /// Validates cognitive-intent invariants.
    pub const fn validate(&self) -> Result<(), CognitiveIntentError> {
        if self.id.is_zero() {
            return Err(CognitiveIntentError::MissingId);
        }
        if self.goal.is_zero() {
            return Err(CognitiveIntentError::MissingGoal);
        }
        if self.target.is_zero() {
            return Err(CognitiveIntentError::MissingTarget);
        }
        if self.evidence.is_zero() {
            return Err(CognitiveIntentError::MissingEvidence);
        }
        if self.requested_rights == 0 {
            return Err(CognitiveIntentError::MissingRequestedRights);
        }
        if self.generation == 0 {
            return Err(CognitiveIntentError::InvalidGeneration);
        }
        if !matches!(self.planner, AuthorityDomain::AgentPlanner) {
            return Err(CognitiveIntentError::PlannerDomainRequired);
        }
        Ok(())
    }
}

/// Errors while converting a plan into an executor permit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionAuthorizationError {
    /// Intent structure is invalid.
    InvalidIntent(CognitiveIntentError),
    /// Capability grant structure is invalid.
    InvalidGrant(CapabilityGrantError),
    /// The capability is not assigned to the isolated agent executor.
    GrantNotForAgentExecutor,
    /// Grant target, rights or lifetime does not authorize the intent.
    GrantDoesNotAuthorizeIntent,
}

/// Capability-checked permit consumed by a future executor.
///
/// The permit contains no ambient authority and cannot exist unless
/// `authorize_intent` succeeds.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionPermitV1 {
    /// Authorized intent.
    pub intent_id: CognitiveIntentId,
    /// Capability that authorized this permit.
    pub capability_id: CapabilityId,
    /// Execution domain.
    pub executor: AuthorityDomain,
    /// Exact semantic target.
    pub target: SemanticObjectId,
    /// Exact rights authorized.
    pub rights: u64,
    /// Generation for which authorization was evaluated.
    pub generation: u64,
}

/// Converts a cognitive proposal into an executor permit only when explicit
/// capability authority covers the exact target, rights and generation.
pub const fn authorize_intent(
    intent: &CognitiveIntentV1,
    grant: &CapabilityGrantV1,
) -> Result<ExecutionPermitV1, ExecutionAuthorizationError> {
    if let Err(error) = intent.validate() {
        return Err(ExecutionAuthorizationError::InvalidIntent(error));
    }
    if let Err(error) = grant.validate() {
        return Err(ExecutionAuthorizationError::InvalidGrant(error));
    }
    if !matches!(grant.subject, AuthorityDomain::AgentExecutor) {
        return Err(ExecutionAuthorizationError::GrantNotForAgentExecutor);
    }
    if !grant.authorizes(intent.target, intent.requested_rights, intent.generation) {
        return Err(ExecutionAuthorizationError::GrantDoesNotAuthorizeIntent);
    }

    Ok(ExecutionPermitV1 {
        intent_id: intent.id,
        capability_id: grant.id,
        executor: AuthorityDomain::AgentExecutor,
        target: intent.target,
        rights: intent.requested_rights,
        generation: intent.generation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scoped_capability(subject: AuthorityDomain, rights: u64) -> CapabilityGrantV1 {
        CapabilityGrantV1::new(
            CapabilityId::new(1),
            CapabilityId::ZERO,
            AuthorityDomain::SystemSovereign,
            subject,
            CapabilityScopeV1::new(SemanticObjectId::new(500), rights, 10, 20),
        )
    }

    #[test]
    fn capability_planner_cannot_receive_mutating_authority() {
        let grant = scoped_capability(
            AuthorityDomain::AgentPlanner,
            capability_right::READ | capability_right::WRITE,
        );
        assert_eq!(
            grant.validate(),
            Err(CapabilityGrantError::PlannerMayNotMutate)
        );
    }

    #[test]
    fn capability_executor_is_explicitly_scoped_and_time_bounded() {
        let grant = scoped_capability(
            AuthorityDomain::AgentExecutor,
            capability_right::READ | capability_right::INVOKE,
        );
        assert_eq!(grant.validate(), Ok(()));
        assert!(grant.authorizes(SemanticObjectId::new(500), capability_right::INVOKE, 15,));
        assert!(!grant.authorizes(SemanticObjectId::new(501), capability_right::INVOKE, 15,));
        assert!(!grant.authorizes(SemanticObjectId::new(500), capability_right::WRITE, 15,));
        assert!(!grant.authorizes(SemanticObjectId::new(500), capability_right::INVOKE, 21,));
    }

    #[test]
    fn capability_kernel_authority_cannot_be_minted_by_human_domain() {
        let grant = scoped_capability(AuthorityDomain::KernelAuthority, capability_right::READ);
        assert_eq!(
            grant.validate(),
            Err(CapabilityGrantError::KernelAuthorityNotDelegable)
        );
    }

    #[test]
    fn capability_system_sovereign_and_kernel_authority_are_distinct() {
        assert_ne!(
            AuthorityDomain::SystemSovereign,
            AuthorityDomain::KernelAuthority
        );
    }

    fn cognitive_intent(requested_rights: u64) -> CognitiveIntentV1 {
        CognitiveIntentV1::new(
            CognitiveIntentId::new(1),
            SemanticObjectId::new(600),
            SemanticObjectId::new(500),
            MemoryRecordId::new(1),
            requested_rights,
            15,
            AuthorityDomain::AgentPlanner,
        )
    }

    #[test]
    fn cognitive_intent_requires_memory_evidence() {
        let intent = CognitiveIntentV1::new(
            CognitiveIntentId::new(1),
            SemanticObjectId::new(600),
            SemanticObjectId::new(500),
            MemoryRecordId::ZERO,
            capability_right::READ,
            15,
            AuthorityDomain::AgentPlanner,
        );
        assert_eq!(
            intent.validate(),
            Err(CognitiveIntentError::MissingEvidence)
        );
    }

    #[test]
    fn cognitive_intent_may_request_mutation_without_owning_authority() {
        let intent = cognitive_intent(capability_right::WRITE);
        assert_eq!(intent.validate(), Ok(()));

        let read_only_grant =
            scoped_capability(AuthorityDomain::AgentExecutor, capability_right::READ);
        assert_eq!(
            authorize_intent(&intent, &read_only_grant),
            Err(ExecutionAuthorizationError::GrantDoesNotAuthorizeIntent)
        );
    }

    #[test]
    fn cognitive_execution_requires_exact_executor_capability() {
        let intent = cognitive_intent(capability_right::INVOKE);
        let executor_grant =
            scoped_capability(AuthorityDomain::AgentExecutor, capability_right::INVOKE);

        let permit = authorize_intent(&intent, &executor_grant).expect("authorized intent");
        assert_eq!(permit.intent_id, intent.id);
        assert_eq!(permit.capability_id, executor_grant.id);
        assert_eq!(permit.executor, AuthorityDomain::AgentExecutor);
        assert_eq!(permit.target, intent.target);
        assert_eq!(permit.rights, capability_right::INVOKE);
        assert_eq!(permit.generation, intent.generation);
    }

    #[test]
    fn cognitive_execution_rejects_non_executor_grant() {
        let intent = cognitive_intent(capability_right::READ);
        let human_grant =
            scoped_capability(AuthorityDomain::SystemSovereign, capability_right::READ);
        assert_eq!(
            authorize_intent(&intent, &human_grant),
            Err(ExecutionAuthorizationError::GrantNotForAgentExecutor)
        );
    }

    fn memory_links() -> MemoryLinksV1 {
        MemoryLinksV1::new(
            SemanticObjectId::new(100),
            SemanticObjectId::new(101),
            SemanticObjectId::new(102),
        )
    }

    #[test]
    fn memory_record_requires_provenance() {
        let record = MemoryRecordV1::new(
            MemoryRecordId::new(1),
            MemoryRecordId::ZERO,
            1,
            MemoryClass::Episodic,
            MemoryRetention::Durable,
            MemoryLinksV1::new(
                SemanticObjectId::new(100),
                SemanticObjectId::new(101),
                SemanticObjectId::ZERO,
            ),
            900_000,
        );

        assert_eq!(record.validate(), Err(MemoryRecordError::MissingProvenance));
    }

    #[test]
    fn memory_record_rejects_invalid_confidence() {
        let record = MemoryRecordV1::new(
            MemoryRecordId::new(1),
            MemoryRecordId::ZERO,
            1,
            MemoryClass::Semantic,
            MemoryRetention::Durable,
            memory_links(),
            1_000_001,
        );

        assert_eq!(record.validate(), Err(MemoryRecordError::InvalidConfidence));
    }

    #[test]
    fn memory_record_supports_versioned_user_controlled_memory() {
        let first = MemoryRecordV1::new(
            MemoryRecordId::new(1),
            MemoryRecordId::ZERO,
            1,
            MemoryClass::Procedural,
            MemoryRetention::UserPinned,
            memory_links(),
            800_000,
        );
        let second = MemoryRecordV1::new(
            MemoryRecordId::new(2),
            first.id,
            2,
            MemoryClass::Procedural,
            MemoryRetention::UserPinned,
            memory_links(),
            900_000,
        );

        assert_eq!(first.validate(), Ok(()));
        assert_eq!(second.validate(), Ok(()));
        assert_eq!(second.previous_version, first.id);
        assert_eq!(second.retention, MemoryRetention::UserPinned);
    }

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
