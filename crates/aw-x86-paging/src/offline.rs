use crate::{MappingError, PageTable, PageTableEntry, PageTableFlags, PhysicalFrame, VirtualPage};

/// Supplies physical frames for page-table storage.
///
/// The allocator remains owned by the caller so the paging crate stays independent from the
/// physical-memory manager implementation.
pub trait FrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysicalFrame>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedMapping {
    pub frame: PhysicalFrame,
    pub flags: PageTableFlags,
}

/// Safe, inactive four-level x86-64 page-table builder.
///
/// Tables are constructed in ordinary Rust memory and associated with physical frames supplied
/// by the caller. The builder never writes physical memory, never loads CR3 and never changes the
/// active address space. A later kernel integration step can materialize the validated table
/// images at their assigned frames.
pub struct OfflinePageTableBuilder<const TABLES: usize> {
    physical_address_bits: u8,
    root_frame: PhysicalFrame,
    table_frames: [Option<PhysicalFrame>; TABLES],
    tables: [PageTable; TABLES],
    table_count: usize,
}

impl<const TABLES: usize> OfflinePageTableBuilder<TABLES> {
    pub fn new<A: FrameAllocator>(
        physical_address_bits: u8,
        allocator: &mut A,
    ) -> Result<Self, MappingError> {
        if !(12..=crate::MAX_X86_64_PHYSICAL_ADDRESS_BITS).contains(&physical_address_bits) {
            return Err(MappingError::InvalidAddress);
        }
        if TABLES == 0 {
            return Err(MappingError::OutOfFrames);
        }

        let root_frame = allocator
            .allocate_frame()
            .ok_or(MappingError::OutOfFrames)?;
        let root_frame = Self::validate_frame(root_frame, physical_address_bits)?;

        let mut table_frames = [None; TABLES];
        table_frames[0] = Some(root_frame);

        Ok(Self {
            physical_address_bits,
            root_frame,
            table_frames,
            tables: core::array::from_fn(|_| PageTable::new()),
            table_count: 1,
        })
    }

    #[must_use]
    pub const fn root_frame(&self) -> PhysicalFrame {
        self.root_frame
    }

    #[must_use]
    pub const fn table_count(&self) -> usize {
        self.table_count
    }

    #[must_use]
    pub const fn root_table(&self) -> &PageTable {
        &self.tables[0]
    }

    #[must_use]
    pub fn table_frame(&self, index: usize) -> Option<PhysicalFrame> {
        if index >= self.table_count {
            return None;
        }
        self.table_frames[index]
    }

    #[must_use]
    pub fn table_for_frame(&self, frame: PhysicalFrame) -> Option<&PageTable> {
        self.table_index_for_frame(frame)
            .map(|index| &self.tables[index])
    }

    /// Map one canonical 4 KiB virtual page to one physical frame.
    ///
    /// Missing PML4/PDPT/PD/PT tables are allocated lazily. Intermediate entries are always
    /// writable so leaf permissions can remain authoritative. USER is propagated upward only
    /// when required. NX stays a leaf property so executable and non-executable 4 KiB pages can
    /// coexist below the same intermediate tables.
    pub fn map_4k<A: FrameAllocator>(
        &mut self,
        allocator: &mut A,
        page: VirtualPage,
        frame: PhysicalFrame,
        flags: PageTableFlags,
    ) -> Result<(), MappingError> {
        let frame = Self::validate_frame(frame, self.physical_address_bits)?;
        let user_accessible = flags.contains(PageTableFlags::USER_ACCESSIBLE);

        let pdpt = self.ensure_child_table(0, page.pml4_index(), user_accessible, allocator)?;
        let pd = self.ensure_child_table(pdpt, page.pdpt_index(), user_accessible, allocator)?;
        let pt = self.ensure_child_table(pd, page.pd_index(), user_accessible, allocator)?;

        self.tables[pt].map_4k_leaf(page.pt_index(), frame, flags)
    }

    /// Resolve a 4 KiB mapping from the inactive hierarchy.
    pub fn resolve_4k(&self, page: VirtualPage) -> Result<ResolvedMapping, MappingError> {
        let pdpt = self.child_table_index(0, page.pml4_index())?;
        let pd = self.child_table_index(pdpt, page.pdpt_index())?;
        let pt = self.child_table_index(pd, page.pd_index())?;

        let entry = self.tables[pt]
            .entry(page.pt_index())
            .ok_or(MappingError::InvalidAddress)?;
        if !entry.is_present() {
            return Err(MappingError::NotMapped);
        }

        let frame = PhysicalFrame::new(entry.frame_address(), self.physical_address_bits)
            .ok_or(MappingError::InvalidAddress)?;
        Ok(ResolvedMapping {
            frame,
            flags: entry.flags(),
        })
    }

    fn ensure_child_table<A: FrameAllocator>(
        &mut self,
        parent_table: usize,
        entry_index: usize,
        user_accessible: bool,
        allocator: &mut A,
    ) -> Result<usize, MappingError> {
        let current = self.tables[parent_table]
            .entry(entry_index)
            .ok_or(MappingError::InvalidAddress)?;

        if current.is_present() {
            let child_frame =
                PhysicalFrame::new(current.frame_address(), self.physical_address_bits)
                    .ok_or(MappingError::InvalidAddress)?;
            let child_index = self
                .table_index_for_frame(child_frame)
                .ok_or(MappingError::InvalidAddress)?;

            if user_accessible && !current.flags().contains(PageTableFlags::USER_ACCESSIBLE) {
                let updated = PageTableEntry::from_frame(
                    child_frame,
                    current.flags().union(PageTableFlags::USER_ACCESSIBLE),
                );
                if !self.tables[parent_table].set_entry(entry_index, updated) {
                    return Err(MappingError::InvalidAddress);
                }
            }

            return Ok(child_index);
        }

        let child_index = self.allocate_table(allocator)?;
        let child_frame = self.table_frames[child_index].ok_or(MappingError::InvalidAddress)?;
        let mut flags = PageTableFlags::PRESENT.union(PageTableFlags::WRITABLE);
        if user_accessible {
            flags = flags.union(PageTableFlags::USER_ACCESSIBLE);
        }

        let entry = PageTableEntry::from_frame(child_frame, flags);
        if !self.tables[parent_table].set_entry(entry_index, entry) {
            return Err(MappingError::InvalidAddress);
        }

        Ok(child_index)
    }

    fn child_table_index(
        &self,
        parent_table: usize,
        entry_index: usize,
    ) -> Result<usize, MappingError> {
        let entry = self.tables[parent_table]
            .entry(entry_index)
            .ok_or(MappingError::InvalidAddress)?;
        if !entry.is_present() {
            return Err(MappingError::NotMapped);
        }

        let frame = PhysicalFrame::new(entry.frame_address(), self.physical_address_bits)
            .ok_or(MappingError::InvalidAddress)?;
        self.table_index_for_frame(frame)
            .ok_or(MappingError::InvalidAddress)
    }

    fn allocate_table<A: FrameAllocator>(
        &mut self,
        allocator: &mut A,
    ) -> Result<usize, MappingError> {
        if self.table_count >= TABLES {
            return Err(MappingError::OutOfFrames);
        }

        let frame = allocator
            .allocate_frame()
            .ok_or(MappingError::OutOfFrames)?;
        let frame = Self::validate_frame(frame, self.physical_address_bits)?;
        if self.table_index_for_frame(frame).is_some() {
            return Err(MappingError::FrameReuse);
        }

        let index = self.table_count;
        self.table_frames[index] = Some(frame);
        self.tables[index] = PageTable::new();
        self.table_count += 1;
        Ok(index)
    }

    fn table_index_for_frame(&self, frame: PhysicalFrame) -> Option<usize> {
        self.table_frames[..self.table_count]
            .iter()
            .position(|candidate| *candidate == Some(frame))
    }

    fn validate_frame(
        frame: PhysicalFrame,
        physical_address_bits: u8,
    ) -> Result<PhysicalFrame, MappingError> {
        PhysicalFrame::new(frame.start_address(), physical_address_bits)
            .ok_or(MappingError::InvalidAddress)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestFrameAllocator {
        next: u64,
        remaining: usize,
    }

    impl TestFrameAllocator {
        fn new(remaining: usize) -> Self {
            Self {
                next: 0x1000_0000,
                remaining,
            }
        }
    }

    impl FrameAllocator for TestFrameAllocator {
        fn allocate_frame(&mut self) -> Option<PhysicalFrame> {
            if self.remaining == 0 {
                return None;
            }

            let frame = PhysicalFrame::new(self.next, 52)?;
            self.next = self.next.checked_add(crate::PAGE_SIZE)?;
            self.remaining -= 1;
            Some(frame)
        }
    }

    struct ReusingFrameAllocator {
        frame: PhysicalFrame,
    }

    impl FrameAllocator for ReusingFrameAllocator {
        fn allocate_frame(&mut self) -> Option<PhysicalFrame> {
            Some(self.frame)
        }
    }

    fn leaf_frame(address: u64) -> PhysicalFrame {
        PhysicalFrame::new(address, 52).unwrap()
    }

    #[test]
    fn builds_and_resolves_single_inactive_four_level_mapping() {
        let mut allocator = TestFrameAllocator::new(8);
        let mut builder = OfflinePageTableBuilder::<8>::new(52, &mut allocator).unwrap();
        let page = VirtualPage::new(0x4000_0000).unwrap();
        let frame = leaf_frame(0x2000_0000);
        let flags = PageTableFlags::WRITABLE.union(PageTableFlags::NO_EXECUTE);

        assert_eq!(builder.table_count(), 1);
        assert_eq!(builder.map_4k(&mut allocator, page, frame, flags), Ok(()));
        assert_eq!(builder.table_count(), 4);

        let mapping = builder.resolve_4k(page).unwrap();
        assert_eq!(mapping.frame, frame);
        assert!(mapping.flags.contains(PageTableFlags::PRESENT));
        assert!(mapping.flags.contains(PageTableFlags::WRITABLE));
        assert!(mapping.flags.contains(PageTableFlags::NO_EXECUTE));
    }

    #[test]
    fn rejects_duplicate_mapping_without_allocating_more_tables() {
        let mut allocator = TestFrameAllocator::new(8);
        let mut builder = OfflinePageTableBuilder::<8>::new(52, &mut allocator).unwrap();
        let page = VirtualPage::new(0x20_0000).unwrap();

        assert_eq!(
            builder.map_4k(
                &mut allocator,
                page,
                leaf_frame(0x2000_0000),
                PageTableFlags::WRITABLE,
            ),
            Ok(())
        );
        let table_count = builder.table_count();
        assert_eq!(
            builder.map_4k(
                &mut allocator,
                page,
                leaf_frame(0x2000_1000),
                PageTableFlags::WRITABLE,
            ),
            Err(MappingError::AlreadyMapped)
        );
        assert_eq!(builder.table_count(), table_count);
    }

    #[test]
    fn reuses_page_tables_for_multiple_pages_in_the_same_pt() {
        let mut allocator = TestFrameAllocator::new(8);
        let mut builder = OfflinePageTableBuilder::<8>::new(52, &mut allocator).unwrap();
        let first = VirtualPage::new(0x20_0000).unwrap();
        let second = VirtualPage::new(0x20_1000).unwrap();

        builder
            .map_4k(
                &mut allocator,
                first,
                leaf_frame(0x2000_0000),
                PageTableFlags::WRITABLE,
            )
            .unwrap();
        builder
            .map_4k(
                &mut allocator,
                second,
                leaf_frame(0x2000_1000),
                PageTableFlags::NO_EXECUTE,
            )
            .unwrap();

        assert_eq!(builder.table_count(), 4);
        assert_eq!(
            builder.resolve_4k(first).unwrap().frame,
            leaf_frame(0x2000_0000)
        );
        assert_eq!(
            builder.resolve_4k(second).unwrap().frame,
            leaf_frame(0x2000_1000)
        );
    }

    #[test]
    fn allocates_new_tables_when_crossing_pt_pd_and_pdpt_boundaries() {
        let mut pt_allocator = TestFrameAllocator::new(8);
        let mut pt_builder = OfflinePageTableBuilder::<8>::new(52, &mut pt_allocator).unwrap();
        pt_builder
            .map_4k(
                &mut pt_allocator,
                VirtualPage::new(0x001f_f000).unwrap(),
                leaf_frame(0x2000_0000),
                PageTableFlags::WRITABLE,
            )
            .unwrap();
        pt_builder
            .map_4k(
                &mut pt_allocator,
                VirtualPage::new(0x0020_0000).unwrap(),
                leaf_frame(0x2000_1000),
                PageTableFlags::WRITABLE,
            )
            .unwrap();
        assert_eq!(pt_builder.table_count(), 5);

        let mut pd_allocator = TestFrameAllocator::new(8);
        let mut pd_builder = OfflinePageTableBuilder::<8>::new(52, &mut pd_allocator).unwrap();
        pd_builder
            .map_4k(
                &mut pd_allocator,
                VirtualPage::new(0x3fff_f000).unwrap(),
                leaf_frame(0x2100_0000),
                PageTableFlags::WRITABLE,
            )
            .unwrap();
        pd_builder
            .map_4k(
                &mut pd_allocator,
                VirtualPage::new(0x4000_0000).unwrap(),
                leaf_frame(0x2100_1000),
                PageTableFlags::WRITABLE,
            )
            .unwrap();
        assert_eq!(pd_builder.table_count(), 6);

        let mut pdpt_allocator = TestFrameAllocator::new(8);
        let mut pdpt_builder = OfflinePageTableBuilder::<8>::new(52, &mut pdpt_allocator).unwrap();
        pdpt_builder
            .map_4k(
                &mut pdpt_allocator,
                VirtualPage::new(0x0000_007f_ffff_f000).unwrap(),
                leaf_frame(0x2200_0000),
                PageTableFlags::WRITABLE,
            )
            .unwrap();
        pdpt_builder
            .map_4k(
                &mut pdpt_allocator,
                VirtualPage::new(0x0000_0080_0000_0000).unwrap(),
                leaf_frame(0x2200_1000),
                PageTableFlags::WRITABLE,
            )
            .unwrap();
        assert_eq!(pdpt_builder.table_count(), 7);
    }

    #[test]
    fn propagates_user_permission_through_intermediate_tables() {
        let mut allocator = TestFrameAllocator::new(8);
        let mut builder = OfflinePageTableBuilder::<8>::new(52, &mut allocator).unwrap();
        let supervisor_page = VirtualPage::new(0x20_0000).unwrap();
        let user_page = VirtualPage::new(0x20_1000).unwrap();

        builder
            .map_4k(
                &mut allocator,
                supervisor_page,
                leaf_frame(0x2000_0000),
                PageTableFlags::WRITABLE,
            )
            .unwrap();
        builder
            .map_4k(
                &mut allocator,
                user_page,
                leaf_frame(0x2000_1000),
                PageTableFlags::USER_ACCESSIBLE,
            )
            .unwrap();

        let pml4_entry = builder.root_table().entry(user_page.pml4_index()).unwrap();
        assert!(pml4_entry.flags().contains(PageTableFlags::USER_ACCESSIBLE));

        let pdpt_frame = PhysicalFrame::new(pml4_entry.frame_address(), 52).unwrap();
        let pdpt = builder.table_for_frame(pdpt_frame).unwrap();
        let pdpt_entry = pdpt.entry(user_page.pdpt_index()).unwrap();
        assert!(pdpt_entry.flags().contains(PageTableFlags::USER_ACCESSIBLE));

        let pd_frame = PhysicalFrame::new(pdpt_entry.frame_address(), 52).unwrap();
        let pd = builder.table_for_frame(pd_frame).unwrap();
        let pd_entry = pd.entry(user_page.pd_index()).unwrap();
        assert!(pd_entry.flags().contains(PageTableFlags::USER_ACCESSIBLE));

        let mapping = builder.resolve_4k(user_page).unwrap();
        assert!(mapping.flags.contains(PageTableFlags::USER_ACCESSIBLE));
        assert!(
            !builder
                .resolve_4k(supervisor_page)
                .unwrap()
                .flags
                .contains(PageTableFlags::USER_ACCESSIBLE)
        );
    }

    #[test]
    fn rejects_allocator_frame_reuse() {
        let frame = leaf_frame(0x1000_0000);
        let mut allocator = ReusingFrameAllocator { frame };
        let mut builder = OfflinePageTableBuilder::<4>::new(52, &mut allocator).unwrap();

        assert_eq!(
            builder.map_4k(
                &mut allocator,
                VirtualPage::new(0x20_0000).unwrap(),
                leaf_frame(0x2000_0000),
                PageTableFlags::WRITABLE,
            ),
            Err(MappingError::FrameReuse)
        );
    }

    #[test]
    fn rejects_out_of_capacity_and_too_wide_leaf_frames() {
        let mut allocator = TestFrameAllocator::new(8);
        let mut builder = OfflinePageTableBuilder::<3>::new(52, &mut allocator).unwrap();
        assert_eq!(
            builder.map_4k(
                &mut allocator,
                VirtualPage::new(0x20_0000).unwrap(),
                leaf_frame(0x2000_0000),
                PageTableFlags::WRITABLE,
            ),
            Err(MappingError::OutOfFrames)
        );

        let mut narrow_allocator = TestFrameAllocator::new(8);
        let mut narrow = OfflinePageTableBuilder::<8>::new(36, &mut narrow_allocator).unwrap();
        let too_wide = PhysicalFrame::new(1_u64 << 40, 52).unwrap();
        assert_eq!(
            narrow.map_4k(
                &mut narrow_allocator,
                VirtualPage::new(0x30_0000).unwrap(),
                too_wide,
                PageTableFlags::WRITABLE,
            ),
            Err(MappingError::InvalidAddress)
        );
    }

    #[test]
    fn never_reuses_allocated_page_table_frames() {
        let mut allocator = TestFrameAllocator::new(8);
        let mut builder = OfflinePageTableBuilder::<8>::new(52, &mut allocator).unwrap();
        builder
            .map_4k(
                &mut allocator,
                VirtualPage::new(0x0000_007f_ffff_f000).unwrap(),
                leaf_frame(0x2000_0000),
                PageTableFlags::WRITABLE,
            )
            .unwrap();
        builder
            .map_4k(
                &mut allocator,
                VirtualPage::new(0x0000_0080_0000_0000).unwrap(),
                leaf_frame(0x2000_1000),
                PageTableFlags::WRITABLE,
            )
            .unwrap();

        for left in 0..builder.table_count() {
            for right in (left + 1)..builder.table_count() {
                assert_ne!(builder.table_frame(left), builder.table_frame(right));
            }
        }
    }

    #[test]
    fn resolve_reports_unmapped_pages() {
        let mut allocator = TestFrameAllocator::new(4);
        let builder = OfflinePageTableBuilder::<4>::new(52, &mut allocator).unwrap();
        assert_eq!(
            builder.resolve_4k(VirtualPage::new(crate::PAGE_SIZE).unwrap()),
            Err(MappingError::NotMapped)
        );
    }
}
