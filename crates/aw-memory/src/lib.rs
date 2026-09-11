#![no_std]
#![forbid(unsafe_code)]

use aw_kernel_core::{MemoryDescriptorHandoff, UEFI_MEMORY_TYPE_CONVENTIONAL, UEFI_PAGE_SIZE};

pub const DEFAULT_BOOTSTRAP_MIN_ADDRESS: u64 = 0x10_0000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalPage {
    start_address: u64,
}

impl PhysicalPage {
    #[must_use]
    pub const fn start_address(self) -> u64 {
        self.start_address
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootstrapAllocatorError {
    EmptyMemoryMap,
    InvalidDescriptor,
    InvalidMinimumAddress,
    NoConventionalMemory,
}

/// Conservative first-stage physical page allocator.
///
/// The allocator only returns pages from UEFI `CONVENTIONAL` descriptors. It
/// deliberately ignores boot-services, ACPI reclaimable and loader memory even
/// though some of those ranges can be reclaimed later. This keeps early page
/// allocation safe until ownership and teardown rules are implemented.
///
/// Descriptors do not need to be sorted. Each allocation scans the complete map
/// and selects the lowest page at or above the monotonically increasing cursor.
pub struct BootstrapPageAllocator<'a> {
    descriptors: &'a [MemoryDescriptorHandoff],
    cursor: u64,
    allocated_pages: u64,
}

impl<'a> BootstrapPageAllocator<'a> {
    pub fn new(
        descriptors: &'a [MemoryDescriptorHandoff],
    ) -> Result<Self, BootstrapAllocatorError> {
        Self::with_minimum_address(descriptors, DEFAULT_BOOTSTRAP_MIN_ADDRESS)
    }

    pub fn with_minimum_address(
        descriptors: &'a [MemoryDescriptorHandoff],
        minimum_address: u64,
    ) -> Result<Self, BootstrapAllocatorError> {
        if descriptors.is_empty() {
            return Err(BootstrapAllocatorError::EmptyMemoryMap);
        }
        if !minimum_address.is_multiple_of(UEFI_PAGE_SIZE) {
            return Err(BootstrapAllocatorError::InvalidMinimumAddress);
        }

        let mut has_usable_conventional = false;
        for descriptor in descriptors {
            if !descriptor.is_valid() {
                return Err(BootstrapAllocatorError::InvalidDescriptor);
            }
            if descriptor.memory_type == UEFI_MEMORY_TYPE_CONVENTIONAL
                && descriptor
                    .physical_end_exclusive()
                    .is_some_and(|end| end > minimum_address)
            {
                has_usable_conventional = true;
            }
        }

        if !has_usable_conventional {
            return Err(BootstrapAllocatorError::NoConventionalMemory);
        }

        Ok(Self {
            descriptors,
            cursor: minimum_address,
            allocated_pages: 0,
        })
    }

    #[must_use]
    pub const fn allocated_pages(&self) -> u64 {
        self.allocated_pages
    }

    pub fn allocate_page(&mut self) -> Option<PhysicalPage> {
        let mut best: Option<u64> = None;

        for descriptor in self.descriptors {
            if descriptor.memory_type != UEFI_MEMORY_TYPE_CONVENTIONAL {
                continue;
            }

            let end = descriptor.physical_end_exclusive()?;
            if end <= self.cursor {
                continue;
            }

            let candidate = descriptor.physical_start.max(self.cursor);
            if candidate >= end {
                continue;
            }

            if best.is_none_or(|current| candidate < current) {
                best = Some(candidate);
            }
        }

        let start_address = best?;
        self.cursor = start_address.checked_add(UEFI_PAGE_SIZE)?;
        self.allocated_pages = self.allocated_pages.checked_add(1)?;
        Some(PhysicalPage { start_address })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn descriptor(memory_type: u32, start: u64, page_count: u64) -> MemoryDescriptorHandoff {
        MemoryDescriptorHandoff {
            memory_type,
            reserved: 0,
            physical_start: start,
            page_count,
            attributes: 0,
        }
    }

    #[test]
    fn allocates_lowest_pages_even_when_map_is_unsorted() {
        let map = [
            descriptor(UEFI_MEMORY_TYPE_CONVENTIONAL, 0x40_0000, 2),
            descriptor(UEFI_MEMORY_TYPE_CONVENTIONAL, 0x10_0000, 2),
            descriptor(UEFI_MEMORY_TYPE_CONVENTIONAL, 0x20_0000, 1),
        ];
        let mut allocator = BootstrapPageAllocator::new(&map).unwrap();

        assert_eq!(
            allocator.allocate_page().unwrap().start_address(),
            0x10_0000
        );
        assert_eq!(
            allocator.allocate_page().unwrap().start_address(),
            0x10_1000
        );
        assert_eq!(
            allocator.allocate_page().unwrap().start_address(),
            0x20_0000
        );
        assert_eq!(
            allocator.allocate_page().unwrap().start_address(),
            0x40_0000
        );
        assert_eq!(
            allocator.allocate_page().unwrap().start_address(),
            0x40_1000
        );
        assert_eq!(allocator.allocate_page(), None);
        assert_eq!(allocator.allocated_pages(), 5);
    }

    #[test]
    fn default_allocator_skips_low_memory() {
        let map = [
            descriptor(UEFI_MEMORY_TYPE_CONVENTIONAL, 0x0, 0x100),
            descriptor(UEFI_MEMORY_TYPE_CONVENTIONAL, 0x10_0000, 1),
        ];
        let mut allocator = BootstrapPageAllocator::new(&map).unwrap();

        assert_eq!(
            allocator.allocate_page().unwrap().start_address(),
            0x10_0000
        );
        assert_eq!(allocator.allocate_page(), None);
    }

    #[test]
    fn custom_minimum_address_is_supported() {
        let map = [descriptor(UEFI_MEMORY_TYPE_CONVENTIONAL, 0x80_0000, 4)];
        let mut allocator = BootstrapPageAllocator::with_minimum_address(&map, 0x80_2000).unwrap();

        assert_eq!(
            allocator.allocate_page().unwrap().start_address(),
            0x80_2000
        );
        assert_eq!(
            allocator.allocate_page().unwrap().start_address(),
            0x80_3000
        );
        assert_eq!(allocator.allocate_page(), None);
    }

    #[test]
    fn ignores_non_conventional_ranges() {
        let map = [
            descriptor(2, 0x10_0000, 8),
            descriptor(11, 0x20_0000, 8),
            descriptor(UEFI_MEMORY_TYPE_CONVENTIONAL, 0x30_0000, 1),
        ];
        let mut allocator = BootstrapPageAllocator::new(&map).unwrap();

        assert_eq!(
            allocator.allocate_page().unwrap().start_address(),
            0x30_0000
        );
        assert_eq!(allocator.allocate_page(), None);
    }

    #[test]
    fn rejects_invalid_descriptors() {
        let map = [MemoryDescriptorHandoff {
            memory_type: UEFI_MEMORY_TYPE_CONVENTIONAL,
            reserved: 0,
            physical_start: 0x10_0001,
            page_count: 1,
            attributes: 0,
        }];

        assert!(matches!(
            BootstrapPageAllocator::new(&map),
            Err(BootstrapAllocatorError::InvalidDescriptor)
        ));
    }

    #[test]
    fn rejects_unaligned_minimum_address() {
        let map = [descriptor(UEFI_MEMORY_TYPE_CONVENTIONAL, 0x10_0000, 1)];

        assert!(matches!(
            BootstrapPageAllocator::with_minimum_address(&map, 0x10_0001),
            Err(BootstrapAllocatorError::InvalidMinimumAddress)
        ));
    }

    #[test]
    fn rejects_map_without_usable_conventional_memory() {
        let map = [
            descriptor(2, 0x10_0000, 1),
            descriptor(11, 0x20_0000, 1),
            descriptor(UEFI_MEMORY_TYPE_CONVENTIONAL, 0x0, 1),
        ];

        assert!(matches!(
            BootstrapPageAllocator::new(&map),
            Err(BootstrapAllocatorError::NoConventionalMemory)
        ));
    }

    #[test]
    fn rejects_empty_map() {
        assert!(matches!(
            BootstrapPageAllocator::new(&[]),
            Err(BootstrapAllocatorError::EmptyMemoryMap)
        ));
    }
}
