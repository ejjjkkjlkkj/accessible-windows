#![no_std]
//! From-zero native persistent-storage foundations.
//!
//! This crate defines only structural contracts. It does not claim that the
//! final filesystem, authenticated storage, encryption or crash recovery are
//! complete.

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
    pub const fn is_zero(self) -> bool {
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
    pub const fn validate_structure(&self) -> Result<(), SuperblockError> {
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
pub const fn newest_structurally_valid<'a>(
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
    pub const fn is_structurally_valid(&self) -> bool {
        !self.transaction_id.is_zero()
            && self.next_generation == self.base_generation.saturating_add(1)
            && self.base_generation != u64::MAX
            && self.new_root.index() != 0
            && !self.new_integrity_root.is_zero()
            && self.reserved == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID_A: ObjectId = ObjectId::new([0xA5; OBJECT_ID_BYTES]);
    const ID_B: ObjectId = ObjectId::new([0x5A; OBJECT_ID_BYTES]);

    #[test]
    fn block_offset_rejects_overflow() {
        assert_eq!(BlockAddress::new(2).byte_offset(), Some(8192));
        assert_eq!(BlockAddress::new(u64::MAX).byte_offset(), None);
    }

    #[test]
    fn falls_back_to_older_anchor_when_newer_is_structurally_corrupt() {
        let old = SuperblockV1::new(41, BlockAddress::new(100), BlockAddress::new(90), ID_A);
        let mut torn = SuperblockV1::new(42, BlockAddress::new(120), BlockAddress::new(100), ID_B);
        torn.magic = 0;

        assert_eq!(newest_structurally_valid(&old, &torn), Some(&old));
    }

    #[test]
    fn selects_newest_valid_generation() {
        let a = SuperblockV1::new(41, BlockAddress::new(100), BlockAddress::new(90), ID_A);
        let b = SuperblockV1::new(42, BlockAddress::new(120), BlockAddress::new(100), ID_B);

        assert_eq!(newest_structurally_valid(&a, &b), Some(&b));
    }

    #[test]
    fn transaction_generation_must_advance_exactly_once() {
        let valid = TransactionRecordV1 {
            transaction_id: ID_A,
            base_generation: 7,
            next_generation: 8,
            new_root: BlockAddress::new(200),
            new_integrity_root: ID_B,
            phase: TransactionPhase::Prepared,
            reserved: 0,
        };
        assert!(valid.is_structurally_valid());

        let invalid = TransactionRecordV1 {
            next_generation: 9,
            ..valid
        };
        assert!(!invalid.is_structurally_valid());
    }
}
