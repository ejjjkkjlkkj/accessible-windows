#![no_std]
//! From-zero native persistent-storage foundations.
//!
//! This crate defines structural and publication contracts. It does not claim
//! that the final filesystem, authenticated storage, encryption or physical
//! crash durability are complete.

/// On-disk magic for the native storage format.
pub const STORAGE_MAGIC: u64 = 0x4157_5354_4F52_4531;
/// First native storage-format version.
pub const STORAGE_FORMAT_VERSION: u32 = 1;
/// Logical block size chosen for the first storage bring-up.
pub const LOGICAL_BLOCK_SIZE: u32 = 4096;
/// Bytes reserved for a native content/integrity identity.
pub const OBJECT_ID_BYTES: usize = 32;

/// Stable identity used by the native storage graph.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ObjectId([u8; OBJECT_ID_BYTES]);

impl ObjectId {
    /// All-zero identity, reserved as invalid/unset.
    pub const ZERO: Self = Self([0; OBJECT_ID_BYTES]);

    /// Builds an identity from raw bytes.
    #[must_use]
    pub const fn new(bytes: [u8; OBJECT_ID_BYTES]) -> Self {
        Self(bytes)
    }

    /// Returns true when the identity is the reserved zero value.
    #[must_use]
    pub fn is_zero(self) -> bool {
        self.0 == [0; OBJECT_ID_BYTES]
    }

    /// Returns the identity bytes.
    #[must_use]
    pub const fn bytes(self) -> [u8; OBJECT_ID_BYTES] {
        self.0
    }
}

/// Logical address inside the native storage space.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct BlockAddress(u64);

impl BlockAddress {
    /// The null block address.
    pub const ZERO: Self = Self(0);

    /// Creates a logical block address.
    #[must_use]
    pub const fn new(index: u64) -> Self {
        Self(index)
    }

    /// Returns the block index.
    #[must_use]
    pub const fn index(self) -> u64 {
        self.0
    }

    /// Returns the byte offset when it can be represented without overflow.
    #[must_use]
    pub fn byte_offset(self) -> Option<u64> {
        self.0.checked_mul(u64::from(LOGICAL_BLOCK_SIZE))
    }
}

/// Structural validation errors for an on-disk anchor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SuperblockError {
    /// Magic does not identify the native storage format.
    BadMagic,
    /// Format version is unsupported.
    UnsupportedVersion,
    /// Structure size does not match version 1.
    BadStructureSize,
    /// Logical block size does not match the version-1 contract.
    BadBlockSize,
    /// Generation zero is reserved for an uninitialized device.
    InvalidGeneration,
    /// No committed root exists.
    MissingCommittedRoot,
    /// No integrity-root identity exists.
    MissingIntegrityRoot,
}

/// Version-1 durable anchor.
///
/// The final format will keep redundant anchors. A newer generation is not
/// trusted merely because its number is larger; structural validation is the
/// first gate and cryptographic integrity verification will be added before the
/// format is declared durable.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SuperblockV1 {
    /// `STORAGE_MAGIC`.
    pub magic: u64,
    /// `STORAGE_FORMAT_VERSION`.
    pub format_version: u32,
    /// Size of this exact structure.
    pub struct_size: u32,
    /// Logical block size.
    pub logical_block_size: u32,
    /// Reserved version-1 flags. Must be zero.
    pub flags: u32,
    /// Monotonically increasing committed generation.
    pub generation: u64,
    /// Root block of the committed native object graph.
    pub committed_root: BlockAddress,
    /// Previous known-good root for explicit recovery.
    pub previous_root: BlockAddress,
    /// Identity of the committed integrity graph.
    pub integrity_root: ObjectId,
}

impl SuperblockV1 {
    /// Creates a structurally valid version-1 anchor.
    #[must_use]
    pub const fn new(
        generation: u64,
        committed_root: BlockAddress,
        previous_root: BlockAddress,
        integrity_root: ObjectId,
    ) -> Self {
        Self {
            magic: STORAGE_MAGIC,
            format_version: STORAGE_FORMAT_VERSION,
            struct_size: core::mem::size_of::<Self>() as u32,
            logical_block_size: LOGICAL_BLOCK_SIZE,
            flags: 0,
            generation,
            committed_root,
            previous_root,
            integrity_root,
        }
    }

    /// Performs version-1 structural validation.
    ///
    /// This is intentionally not called cryptographic verification.
    pub fn validate_structure(&self) -> Result<(), SuperblockError> {
        if self.magic != STORAGE_MAGIC {
            return Err(SuperblockError::BadMagic);
        }
        if self.format_version != STORAGE_FORMAT_VERSION {
            return Err(SuperblockError::UnsupportedVersion);
        }
        if self.struct_size != core::mem::size_of::<Self>() as u32 || self.flags != 0 {
            return Err(SuperblockError::BadStructureSize);
        }
        if self.logical_block_size != LOGICAL_BLOCK_SIZE {
            return Err(SuperblockError::BadBlockSize);
        }
        if self.generation == 0 {
            return Err(SuperblockError::InvalidGeneration);
        }
        if self.committed_root.index() == 0 {
            return Err(SuperblockError::MissingCommittedRoot);
        }
        if self.integrity_root.is_zero() {
            return Err(SuperblockError::MissingIntegrityRoot);
        }
        Ok(())
    }
}

/// Chooses the newest structurally valid redundant anchor.
///
/// Equal generations deliberately select `a` deterministically. Cryptographic
/// verification and write-order evidence will be added before this function is
/// used for production recovery.
#[must_use]
pub fn newest_structurally_valid<'a>(
    a: &'a SuperblockV1,
    b: &'a SuperblockV1,
) -> Option<&'a SuperblockV1> {
    let a_valid = a.validate_structure().is_ok();
    let b_valid = b.validate_structure().is_ok();

    match (a_valid, b_valid) {
        (false, false) => None,
        (true, false) => Some(a),
        (false, true) => Some(b),
        (true, true) => {
            if b.generation > a.generation {
                Some(b)
            } else {
                Some(a)
            }
        }
    }
}

/// Durable transaction phase.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransactionPhase {
    /// Intent is durable but not yet published as the active root.
    Prepared = 1,
    /// New root is committed.
    Committed = 2,
    /// Transaction has been explicitly abandoned.
    Aborted = 3,
}

/// Version-1 transaction descriptor.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransactionRecordV1 {
    /// Stable transaction identity.
    pub transaction_id: ObjectId,
    /// Generation from which this transaction starts.
    pub base_generation: u64,
    /// Generation produced if committed.
    pub next_generation: u64,
    /// Candidate new root.
    pub new_root: BlockAddress,
    /// Candidate integrity-root identity.
    pub new_integrity_root: ObjectId,
    /// Durable transaction phase.
    pub phase: TransactionPhase,
    /// Reserved for alignment and future flags. Must be zero.
    pub reserved: u32,
}

impl TransactionRecordV1 {
    /// Returns true when the descriptor obeys version-1 monotonicity rules.
    #[must_use]
    pub fn is_structurally_valid(&self) -> bool {
        !self.transaction_id.is_zero()
            && self.next_generation == self.base_generation.saturating_add(1)
            && self.base_generation != u64::MAX
            && self.new_root.index() != 0
            && !self.new_integrity_root.is_zero()
            && self.reserved == 0
    }
}

/// Error returned while deriving an anchor that could publish a prepared transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationError {
    /// The currently active anchor is structurally invalid.
    CurrentAnchorInvalid(SuperblockError),
    /// The transaction descriptor violates version-1 structural rules.
    InvalidTransaction,
    /// Only a prepared transaction may be published.
    TransactionNotPrepared,
    /// The transaction does not advance exactly from the current generation.
    GenerationMismatch,
}

/// Derives the next durable anchor without mutating the current known-good anchor.
///
/// This function models publication only. It deliberately performs no physical
/// writes and makes no durability claim. The returned anchor preserves the
/// current committed root as its explicit recovery root.
pub fn prepare_publication(
    current: &SuperblockV1,
    transaction: &TransactionRecordV1,
) -> Result<SuperblockV1, PublicationError> {
    current
        .validate_structure()
        .map_err(PublicationError::CurrentAnchorInvalid)?;

    if !transaction.is_structurally_valid() {
        return Err(PublicationError::InvalidTransaction);
    }
    if transaction.phase != TransactionPhase::Prepared {
        return Err(PublicationError::TransactionNotPrepared);
    }

    let expected_next = current
        .generation
        .checked_add(1)
        .ok_or(PublicationError::GenerationMismatch)?;
    if transaction.base_generation != current.generation
        || transaction.next_generation != expected_next
    {
        return Err(PublicationError::GenerationMismatch);
    }

    Ok(SuperblockV1::new(
        transaction.next_generation,
        transaction.new_root,
        current.committed_root,
        transaction.new_integrity_root,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID_A: ObjectId = ObjectId::new([0xA5; OBJECT_ID_BYTES]);
    const ID_B: ObjectId = ObjectId::new([0x5A; OBJECT_ID_BYTES]);
    const TX_ID: ObjectId = ObjectId::new([0xC3; OBJECT_ID_BYTES]);

    fn anchor(generation: u64, root: u64, previous: u64, id: ObjectId) -> SuperblockV1 {
        SuperblockV1::new(
            generation,
            BlockAddress::new(root),
            BlockAddress::new(previous),
            id,
        )
    }

    fn prepared_transaction(base: u64, next: u64, root: u64) -> TransactionRecordV1 {
        TransactionRecordV1 {
            transaction_id: TX_ID,
            base_generation: base,
            next_generation: next,
            new_root: BlockAddress::new(root),
            new_integrity_root: ID_B,
            phase: TransactionPhase::Prepared,
            reserved: 0,
        }
    }

    #[test]
    fn block_offset_rejects_overflow() {
        assert_eq!(BlockAddress::new(2).byte_offset(), Some(8192));
        assert_eq!(BlockAddress::new(u64::MAX).byte_offset(), None);
    }

    #[test]
    fn recovery_falls_back_when_newer_anchor_is_structurally_corrupt() {
        let old = anchor(41, 100, 90, ID_A);
        let mut torn = anchor(42, 120, 100, ID_B);
        torn.magic = 0;

        assert_eq!(newest_structurally_valid(&old, &torn), Some(&old));
    }

    #[test]
    fn recovery_selects_newest_valid_generation() {
        let a = anchor(41, 100, 90, ID_A);
        let b = anchor(42, 120, 100, ID_B);

        assert_eq!(newest_structurally_valid(&a, &b), Some(&b));
    }

    #[test]
    fn transaction_generation_must_advance_exactly_once() {
        let valid = prepared_transaction(7, 8, 200);
        assert!(valid.is_structurally_valid());

        let invalid = TransactionRecordV1 {
            next_generation: 9,
            ..valid
        };
        assert!(!invalid.is_structurally_valid());
    }

    #[test]
    fn publication_preserves_previous_known_good_root() {
        let current = anchor(41, 100, 90, ID_A);
        let transaction = prepared_transaction(41, 42, 120);

        let next = prepare_publication(&current, &transaction).expect("valid publication");
        assert_eq!(next.generation, 42);
        assert_eq!(next.committed_root, BlockAddress::new(120));
        assert_eq!(next.previous_root, current.committed_root);
        assert_eq!(next.integrity_root, ID_B);
    }

    #[test]
    fn publication_rejects_stale_transaction() {
        let current = anchor(42, 120, 100, ID_B);
        let stale = prepared_transaction(41, 42, 130);

        assert_eq!(
            prepare_publication(&current, &stale),
            Err(PublicationError::GenerationMismatch)
        );
    }

    #[test]
    fn publication_rejects_non_prepared_transaction() {
        let current = anchor(41, 100, 90, ID_A);
        let transaction = TransactionRecordV1 {
            phase: TransactionPhase::Committed,
            ..prepared_transaction(41, 42, 120)
        };

        assert_eq!(
            prepare_publication(&current, &transaction),
            Err(PublicationError::TransactionNotPrepared)
        );
    }

    #[test]
    fn recovery_changes_generation_only_after_valid_anchor_publication() {
        let stable = anchor(41, 100, 90, ID_A);
        let transaction = prepared_transaction(41, 42, 120);

        let mut unpublished_slot = prepare_publication(&stable, &transaction).expect("candidate");
        unpublished_slot.magic = 0;
        assert_eq!(
            newest_structurally_valid(&stable, &unpublished_slot),
            Some(&stable)
        );

        let published = prepare_publication(&stable, &transaction).expect("published candidate");
        assert_eq!(
            newest_structurally_valid(&stable, &published),
            Some(&published)
        );
    }

    #[derive(Clone, Copy)]
    struct PersistenceModel {
        anchor_a: SuperblockV1,
        anchor_b: SuperblockV1,
        candidate_staged: bool,
        candidate_durable: bool,
        transaction_staged: bool,
        transaction_durable: bool,
        staged_anchor_b: Option<SuperblockV1>,
    }

    impl PersistenceModel {
        fn new(anchor_a: SuperblockV1, anchor_b: SuperblockV1) -> Self {
            Self {
                anchor_a,
                anchor_b,
                candidate_staged: false,
                candidate_durable: false,
                transaction_staged: false,
                transaction_durable: false,
                staged_anchor_b: None,
            }
        }

        fn stage_candidate_objects(&mut self) {
            self.candidate_staged = true;
        }

        fn stage_prepared_transaction(&mut self) {
            self.transaction_staged = true;
        }

        fn stage_anchor(&mut self, anchor: SuperblockV1) {
            self.staged_anchor_b = Some(anchor);
        }

        fn durability_barrier(&mut self) {
            if self.candidate_staged {
                self.candidate_durable = true;
                self.candidate_staged = false;
            }
            if self.transaction_staged {
                self.transaction_durable = true;
                self.transaction_staged = false;
            }
            if let Some(anchor) = self.staged_anchor_b.take() {
                self.anchor_b = anchor;
            }
        }

        fn crash(mut self) -> Self {
            self.candidate_staged = false;
            self.transaction_staged = false;
            self.staged_anchor_b = None;
            self
        }

        fn recovered_generation(&self) -> Option<u64> {
            newest_structurally_valid(&self.anchor_a, &self.anchor_b)
                .map(|anchor| anchor.generation)
        }
    }

    fn assert_crash_recovers(model: PersistenceModel, expected_generation: u64) {
        let crashed = model.crash();
        assert_eq!(crashed.recovered_generation(), Some(expected_generation));
    }

    #[test]
    fn crash_cut_model_requires_durable_anchor_publication() {
        let stable = anchor(41, 100, 90, ID_A);
        let transaction = prepared_transaction(41, 42, 120);
        let published = prepare_publication(&stable, &transaction).expect("publication candidate");

        let mut invalid_spare = anchor(1, 1, 1, ID_A);
        invalid_spare.magic = 0;

        let mut model = PersistenceModel::new(stable, invalid_spare);
        assert_crash_recovers(model, 41);

        model.stage_candidate_objects();
        assert_crash_recovers(model, 41);

        model.durability_barrier();
        assert!(model.candidate_durable);
        assert_crash_recovers(model, 41);

        model.stage_prepared_transaction();
        assert_crash_recovers(model, 41);

        model.durability_barrier();
        assert!(model.transaction_durable);
        assert_crash_recovers(model, 41);

        model.stage_anchor(published);
        assert_crash_recovers(model, 41);

        model.durability_barrier();
        assert_eq!(model.recovered_generation(), Some(42));
        assert_crash_recovers(model, 42);
    }
}
