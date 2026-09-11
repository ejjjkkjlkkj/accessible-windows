#![no_std]
#![forbid(unsafe_code)]

pub const PAGE_SIZE: u64 = 4096;
pub const PAGE_TABLE_ENTRIES: usize = 512;
pub const MAX_X86_64_PHYSICAL_ADDRESS_BITS: u8 = 52;
pub const PAGE_FRAME_ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtualAddress(u64);

impl VirtualAddress {
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        let upper = value >> 48;
        let sign = (value >> 47) & 1;
        if (sign == 0 && upper == 0) || (sign == 1 && upper == 0xffff) {
            Some(Self(value))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn is_page_aligned(self) -> bool {
        self.0 & (PAGE_SIZE - 1) == 0
    }

    #[must_use]
    pub const fn page_offset(self) -> usize {
        (self.0 & (PAGE_SIZE - 1)) as usize
    }

    #[must_use]
    pub const fn pml4_index(self) -> usize {
        ((self.0 >> 39) & 0x1ff) as usize
    }

    #[must_use]
    pub const fn pdpt_index(self) -> usize {
        ((self.0 >> 30) & 0x1ff) as usize
    }

    #[must_use]
    pub const fn pd_index(self) -> usize {
        ((self.0 >> 21) & 0x1ff) as usize
    }

    #[must_use]
    pub const fn pt_index(self) -> usize {
        ((self.0 >> 12) & 0x1ff) as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalFrame {
    start_address: u64,
}

impl PhysicalFrame {
    #[must_use]
    pub const fn new(start_address: u64, physical_address_bits: u8) -> Option<Self> {
        if physical_address_bits < 12
            || physical_address_bits > MAX_X86_64_PHYSICAL_ADDRESS_BITS
            || start_address & (PAGE_SIZE - 1) != 0
        {
            return None;
        }

        let limit = 1_u64 << physical_address_bits;
        if start_address >= limit {
            return None;
        }

        Some(Self { start_address })
    }

    #[must_use]
    pub const fn start_address(self) -> u64 {
        self.start_address
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PageTableFlags(u64);

impl PageTableFlags {
    pub const PRESENT: Self = Self(1 << 0);
    pub const WRITABLE: Self = Self(1 << 1);
    pub const USER_ACCESSIBLE: Self = Self(1 << 2);
    pub const WRITE_THROUGH: Self = Self(1 << 3);
    pub const CACHE_DISABLE: Self = Self(1 << 4);
    pub const ACCESSED: Self = Self(1 << 5);
    pub const DIRTY: Self = Self(1 << 6);
    pub const HUGE_PAGE: Self = Self(1 << 7);
    pub const GLOBAL: Self = Self(1 << 8);
    pub const NO_EXECUTE: Self = Self(1 << 63);

    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn bits(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn from_frame(frame: PhysicalFrame, flags: PageTableFlags) -> Self {
        Self((frame.start_address() & PAGE_FRAME_ADDRESS_MASK) | flags.bits())
    }

    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn frame_address(self) -> u64 {
        self.0 & PAGE_FRAME_ADDRESS_MASK
    }

    #[must_use]
    pub const fn flags(self) -> PageTableFlags {
        PageTableFlags(self.0 & !PAGE_FRAME_ADDRESS_MASK)
    }

    #[must_use]
    pub const fn is_present(self) -> bool {
        self.flags().contains(PageTableFlags::PRESENT)
    }
}

#[repr(C, align(4096))]
pub struct PageTable {
    entries: [PageTableEntry; PAGE_TABLE_ENTRIES],
}

impl PageTable {
    pub const fn new() -> Self {
        Self {
            entries: [PageTableEntry::empty(); PAGE_TABLE_ENTRIES],
        }
    }

    #[must_use]
    pub const fn entry(&self, index: usize) -> Option<PageTableEntry> {
        if index < PAGE_TABLE_ENTRIES {
            Some(self.entries[index])
        } else {
            None
        }
    }

    pub fn set_entry(&mut self, index: usize, entry: PageTableEntry) -> bool {
        if index >= PAGE_TABLE_ENTRIES {
            return false;
        }
        self.entries[index] = entry;
        true
    }
}

impl Default for PageTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_canonical_48_bit_virtual_addresses() {
        assert!(VirtualAddress::new(0x0000_7fff_ffff_ffff).is_some());
        assert!(VirtualAddress::new(0xffff_8000_0000_0000).is_some());
        assert!(VirtualAddress::new(0x0000_8000_0000_0000).is_none());
        assert!(VirtualAddress::new(0xffff_7fff_ffff_ffff).is_none());
    }

    #[test]
    fn decomposes_virtual_address_into_four_level_indices() {
        let address = VirtualAddress::new(0xffff_8123_4567_89ab).unwrap();
        assert_eq!(address.pml4_index(), 0x102);
        assert_eq!(address.pdpt_index(), 0x08d);
        assert_eq!(address.pd_index(), 0x02b);
        assert_eq!(address.pt_index(), 0x078);
        assert_eq!(address.page_offset(), 0x9ab);
    }

    #[test]
    fn validates_physical_frames_against_reported_address_width() {
        assert!(PhysicalFrame::new(0x0010_0000, 36).is_some());
        assert!(PhysicalFrame::new(0x0010_0001, 36).is_none());
        assert!(PhysicalFrame::new(1_u64 << 36, 36).is_none());
        assert!(PhysicalFrame::new(0, 11).is_none());
        assert!(PhysicalFrame::new(0, 53).is_none());
    }

    #[test]
    fn encodes_frame_address_and_entry_flags_without_overlap() {
        let frame = PhysicalFrame::new(0x0000_1234_5678_9000, 52).unwrap();
        let flags = PageTableFlags::PRESENT
            .union(PageTableFlags::WRITABLE)
            .union(PageTableFlags::NO_EXECUTE);
        let entry = PageTableEntry::from_frame(frame, flags);

        assert_eq!(entry.frame_address(), 0x0000_1234_5678_9000);
        assert!(entry.is_present());
        assert!(entry.flags().contains(PageTableFlags::WRITABLE));
        assert!(entry.flags().contains(PageTableFlags::NO_EXECUTE));
    }

    #[test]
    fn page_table_is_exactly_one_page_and_page_aligned() {
        assert_eq!(core::mem::size_of::<PageTable>(), PAGE_SIZE as usize);
        assert_eq!(core::mem::align_of::<PageTable>(), PAGE_SIZE as usize);
    }

    #[test]
    fn page_table_bounds_checks_entries() {
        let mut table = PageTable::new();
        let frame = PhysicalFrame::new(0x20_0000, 52).unwrap();
        let entry = PageTableEntry::from_frame(frame, PageTableFlags::PRESENT);

        assert!(table.set_entry(511, entry));
        assert_eq!(table.entry(511), Some(entry));
        assert!(!table.set_entry(512, entry));
        assert_eq!(table.entry(512), None);
    }
}
