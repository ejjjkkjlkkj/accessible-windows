//! Minimal virtual-memory bring-up: take over paging from the firmware.
//!
//! The freestanding kernel starts out running on the page tables UEFI left
//! active. This module builds a kernel-owned four-level page hierarchy that
//! identity-maps the low [`IDENTITY_GIB`] GiB with 1 GiB huge pages and loads
//! it into CR3. Identity mapping keeps every physical address at the same
//! virtual address, so the currently-executing code, the active stack, the
//! framebuffer and the PCIe ECAM window all remain valid across the switch --
//! this is the first step of an eventual full virtual-memory manager, not the
//! final address-space layout.

#![allow(dead_code)]

use core::arch::asm;
use core::arch::x86_64::__cpuid;

use aw_x86_paging::{
    MAX_X86_64_PHYSICAL_ADDRESS_BITS, PageTable, PhysicalFrame, identity_map_low_gib,
};

/// Size of the low identity window installed at bring-up. 4 GiB covers all
/// conventional RAM, the framebuffer and the PCIe ECAM region on the supported
/// platforms while costing a single PML4 plus a single PDPT.
pub const IDENTITY_GIB: u64 = 4;

/// Number of 8-byte entries in one page table.
const PAGE_TABLE_ENTRIES: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VmmError {
    /// The CPU does not advertise 1 GiB pages (CPUID.80000001H:EDX[26]).
    NoOneGibPages,
    /// A supplied page-table frame was null, misaligned or duplicated.
    BadTableFrame,
    /// The pure identity-map builder rejected the requested window.
    BuildFailed,
}

impl VmmError {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::NoOneGibPages => "no-1gib-pages",
            Self::BadTableFrame => "bad-table-frame",
            Self::BuildFailed => "build-failed",
        }
    }
}

/// Whether the CPU supports 1 GiB pages (CPUID.80000001H:EDX bit 26).
#[must_use]
pub fn supports_1gib_pages() -> bool {
    // Extended leaf 0x8000_0001 is architectural on every x86-64 CPU this
    // kernel targets; `__cpuid` is safe on this always-available target.
    let leaf = __cpuid(0x8000_0001);
    (leaf.edx & (1 << 26)) != 0
}

/// Read the active CR3 (physical address of the top-level page table).
#[must_use]
pub fn current_cr3() -> u64 {
    let value: u64;
    // SAFETY: reading CR3 at CPL0 has no side effects.
    unsafe {
        asm!("mov {}, cr3", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

/// Build the low identity map in `pml4_frame`/`pdpt_frame` and load CR3.
///
/// Returns the new CR3 value (the PML4 physical address) on success.
///
/// # Safety
///
/// Must run once at CPL0 during single-core bootstrap. `pml4_frame` and
/// `pdpt_frame` must be two distinct, 4 KiB-aligned, currently-writable free
/// RAM frames located inside the identity window. On return the kernel is
/// executing on kernel-owned page tables and the firmware tables are no longer
/// referenced.
pub unsafe fn activate_identity_map(pml4_frame: u64, pdpt_frame: u64) -> Result<u64, VmmError> {
    if !supports_1gib_pages() {
        return Err(VmmError::NoOneGibPages);
    }
    if pml4_frame == 0
        || pdpt_frame == 0
        || pml4_frame == pdpt_frame
        || pml4_frame & 0xfff != 0
        || pdpt_frame & 0xfff != 0
    {
        return Err(VmmError::BadTableFrame);
    }

    let pdpt_physical = PhysicalFrame::new(pdpt_frame, MAX_X86_64_PHYSICAL_ADDRESS_BITS)
        .ok_or(VmmError::BadTableFrame)?;

    let mut pml4 = PageTable::new();
    let mut pdpt = PageTable::new();
    identity_map_low_gib(&mut pml4, &mut pdpt, pdpt_physical, IDENTITY_GIB)
        .map_err(|_| VmmError::BuildFailed)?;

    // SAFETY: both frames are distinct, page-aligned, writable RAM still
    // reachable through the firmware identity map; after materializing the two
    // tables we point CR3 at the freshly written PML4, which also flushes the
    // TLB.
    unsafe {
        materialize_table(pml4_frame, &pml4);
        materialize_table(pdpt_frame, &pdpt);
        asm!("mov cr3, {}", in(reg) pml4_frame, options(nostack, preserves_flags));
    }

    Ok(pml4_frame)
}

/// Copy a built page table into its physical frame as 512 raw 64-bit entries.
///
/// # Safety
/// `frame` must be a 4 KiB-aligned writable RAM frame reachable at its physical
/// address (true while the firmware identity map is still active).
unsafe fn materialize_table(frame: u64, table: &PageTable) {
    let destination = frame as *mut u64;
    let mut index = 0;
    while index < PAGE_TABLE_ENTRIES {
        let raw = match table.entry(index) {
            Some(entry) => entry.raw(),
            None => 0,
        };
        // SAFETY: `index` stays below 512, so the write lands inside the frame.
        unsafe {
            core::ptr::write_volatile(destination.add(index), raw);
        }
        index += 1;
    }
}
