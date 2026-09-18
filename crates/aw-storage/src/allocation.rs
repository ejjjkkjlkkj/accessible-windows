//! Native allocation contracts for the from-zero storage line.
//!
//! This module defines structural block-ownership invariants. It is not yet a
//! persistent free-space allocator.

use super::{BlockAddress, ObjectId};

/// Structural errors for a contiguous logical-block extent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExtentError {
    /// Block zero is reserved and cannot be allocated.
    ReservedBlockZero,
    /// An allocation must contain at least one logical block.
    EmptyExtent,
    /// The exclusive end block cannot be represented in `u64`.
    AddressOverflow,
}

/// Contiguous logical-block extent.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExtentV1 {
    /// First logical block in the extent.
    pub start: BlockAddress,
    /// Number of logical blocks in the extent.
    pub block_count: u64,
}

impl ExtentV1 {
    /// Creates an extent.
    #[must_use]
    pub const fn new(start: BlockAddress, block_count: u64) -> Self {
        Self { start, block_count }
    }

    /// Returns the exclusive end block after structural validation.
    pub fn end_exclusive(self) -> Result<u64, ExtentError> {
        if self.start.index() == 0 {
            return Err(ExtentError::ReservedBlockZero);
        }
        if self.block_count == 0 {
            return Err(ExtentError::EmptyExtent);
        }
        self.start
            .index()
            .checked_add(self.block_count)
            .ok_or(ExtentError::AddressOverflow)
    }

    /// Validates the extent's version-1 structural invariants.
    pub fn validate_structure(self) -> Result<(), ExtentError> {
        self.end_exclusive().map(|_| ())
    }

    /// Returns whether two structurally valid extents overlap.
    pub fn overlaps(self, other: Self) -> Result<bool, ExtentError> {
        let self_end = self.end_exclusive()?;
        let other_end = other.end_exclusive()?;
        Ok(self.start.index() < other_end && other.start.index() < self_end)
    }
}

/// Structural validation errors for a native allocation record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AllocationRecordError {
    /// Every allocation record requires a stable nonzero identity.
    MissingAllocationId,
    /// Every allocation must identify its owning native object.
    MissingOwner,
    /// Generation zero is reserved for uninitialized storage.
    InvalidGeneration,
    /// Reserved fields must remain zero in version 1.
    ReservedFieldsSet,
    /// The owned extent is structurally invalid.
    InvalidExtent(ExtentError),
}

/// Immutable version-1 block-allocation record.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AllocationRecordV1 {
    /// Stable identity of this allocation record.
    pub allocation_id: ObjectId,
    /// Native object that owns the allocated blocks.
    pub owner: ObjectId,
    /// Storage generation in which the allocation became valid.
    pub generation: u64,
    /// Contiguous logical-block extent owned by `owner`.
    pub extent: ExtentV1,
    /// Reserved version-1 flags. Must be zero.
    pub flags: u32,
    /// Reserved alignment/future field. Must be zero.
    pub reserved: u32,
}

impl AllocationRecordV1 {
    /// Creates a version-1 allocation record.
    #[must_use]
    pub const fn new(
        allocation_id: ObjectId,
        owner: ObjectId,
        generation: u64,
        extent: ExtentV1,
    ) -> Self {
        Self {
            allocation_id,
            owner,
            generation,
            extent,
            flags: 0,
            reserved: 0,
        }
    }

    /// Validates version-1 allocation-record invariants.
    pub fn validate_structure(&self) -> Result<(), AllocationRecordError> {
        if self.allocation_id.is_zero() {
            return Err(AllocationRecordError::MissingAllocationId);
        }
        if self.owner.is_zero() {
            return Err(AllocationRecordError::MissingOwner);
        }
        if self.generation == 0 {
            return Err(AllocationRecordError::InvalidGeneration);
        }
        if self.flags != 0 || self.reserved != 0 {
            return Err(AllocationRecordError::ReservedFieldsSet);
        }
        self.extent
            .validate_structure()
            .map_err(AllocationRecordError::InvalidExtent)
    }
}

/// Errors when validating two allocations that must coexist.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AllocationPairError {
    /// The first allocation record is structurally invalid.
    FirstInvalid(AllocationRecordError),
    /// The second allocation record is structurally invalid.
    SecondInvalid(AllocationRecordError),
    /// Two live allocation records claim overlapping logical blocks.
    Overlap,
}

/// Validates that two allocation records are structurally valid and disjoint.
pub fn validate_disjoint_allocations(
    first: &AllocationRecordV1,
    second: &AllocationRecordV1,
) -> Result<(), AllocationPairError> {
    first
        .validate_structure()
        .map_err(AllocationPairError::FirstInvalid)?;
    second
        .validate_structure()
        .map_err(AllocationPairError::SecondInvalid)?;

    if first.extent.overlaps(second.extent).map_err(|error| {
        AllocationPairError::FirstInvalid(AllocationRecordError::InvalidExtent(error))
    })? {
        return Err(AllocationPairError::Overlap);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OBJECT_ID_BYTES;

    const ALLOCATION_A: ObjectId = ObjectId::new([0x11; OBJECT_ID_BYTES]);
    const ALLOCATION_B: ObjectId = ObjectId::new([0x22; OBJECT_ID_BYTES]);
    const OWNER_A: ObjectId = ObjectId::new([0xA1; OBJECT_ID_BYTES]);
    const OWNER_B: ObjectId = ObjectId::new([0xB2; OBJECT_ID_BYTES]);

    fn record(id: ObjectId, owner: ObjectId, start: u64, count: u64) -> AllocationRecordV1 {
        AllocationRecordV1::new(id, owner, 7, ExtentV1::new(BlockAddress::new(start), count))
    }

    #[test]
    fn allocation_rejects_reserved_block_zero() {
        let allocation = record(ALLOCATION_A, OWNER_A, 0, 1);
        assert_eq!(
            allocation.validate_structure(),
            Err(AllocationRecordError::InvalidExtent(
                ExtentError::ReservedBlockZero
            ))
        );
    }

    #[test]
    fn allocation_rejects_empty_extent() {
        let allocation = record(ALLOCATION_A, OWNER_A, 10, 0);
        assert_eq!(
            allocation.validate_structure(),
            Err(AllocationRecordError::InvalidExtent(
                ExtentError::EmptyExtent
            ))
        );
    }

    #[test]
    fn allocation_rejects_address_overflow() {
        let allocation = record(ALLOCATION_A, OWNER_A, u64::MAX, 1);
        assert_eq!(
            allocation.validate_structure(),
            Err(AllocationRecordError::InvalidExtent(
                ExtentError::AddressOverflow
            ))
        );
    }

    #[test]
    fn allocation_detects_overlap() {
        let first = record(ALLOCATION_A, OWNER_A, 100, 8);
        let second = record(ALLOCATION_B, OWNER_B, 107, 4);
        assert_eq!(
            validate_disjoint_allocations(&first, &second),
            Err(AllocationPairError::Overlap)
        );
    }

    #[test]
    fn allocation_accepts_adjacent_extents() {
        let first = record(ALLOCATION_A, OWNER_A, 100, 8);
        let second = record(ALLOCATION_B, OWNER_B, 108, 4);
        assert_eq!(validate_disjoint_allocations(&first, &second), Ok(()));
    }

    #[test]
    fn allocation_requires_owner_and_generation() {
        let missing_owner = record(ALLOCATION_A, ObjectId::ZERO, 100, 8);
        assert_eq!(
            missing_owner.validate_structure(),
            Err(AllocationRecordError::MissingOwner)
        );

        let invalid_generation = AllocationRecordV1 {
            generation: 0,
            ..record(ALLOCATION_A, OWNER_A, 100, 8)
        };
        assert_eq!(
            invalid_generation.validate_structure(),
            Err(AllocationRecordError::InvalidGeneration)
        );
    }
}
