//! Minimal NVMe driver: bring up the controller and read its IDENTIFY data
//! (dossier section 11.2 "NVMe/AHCI", roadmap Phase 3 "NVMe controller
//! initialization and identify").
//!
//! AHCI proved a real SATA controller; this proves the interface modern PCs boot
//! their SSDs through. NVMe is memory-mapped, not port-mapped: everything happens
//! through a BAR0 register block and a pair of DMA queues in RAM. The proof sets
//! up the admin submission/completion queues, enables the controller, issues one
//! IDENTIFY CONTROLLER command, and reads the model number the controller wrote
//! back by DMA - real content the device produced, not a status bit.
//!
//! Only what a single admin command needs is implemented: the admin queue pair,
//! one command, polled completion by the CQ phase tag, no interrupts, no I/O
//! queues, no namespaces. Every structure the controller touches by DMA lives in
//! a page-aligned `static` the identity map covers 1:1, so its virtual address is
//! also the physical address handed to the controller. IDENTIFY is read-only and
//! safe on any machine, so this runs on the normal boot path.

use aw_x86_paging::PageTableFlags;

use crate::page_mapper::map_page;
use crate::{debug_write, debug_write_hex_u64, debug_write_u64};

// ---- x86 port I/O for PCI mechanism #1 (CF8/CFC) ------------------------

unsafe fn outl(port: u16, value: u32) {
    // SAFETY: the caller names a valid dword-wide port.
    unsafe {
        core::arch::asm!("out dx, eax", in("dx") port, in("eax") value,
            options(nomem, nostack, preserves_flags));
    }
}

unsafe fn inl(port: u16) -> u32 {
    let value: u32;
    // SAFETY: the caller names a valid dword-wide port.
    unsafe {
        core::arch::asm!("in eax, dx", out("eax") value, in("dx") port,
            options(nomem, nostack, preserves_flags));
    }
    value
}

const PCI_CONFIG_ADDRESS: u16 = 0x0cf8;
const PCI_CONFIG_DATA: u16 = 0x0cfc;

fn pci_address(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    0x8000_0000
        | (u32::from(bus) << 16)
        | (u32::from(device) << 11)
        | (u32::from(function) << 8)
        | u32::from(offset & 0xfc)
}

unsafe fn pci_read32(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    // SAFETY: CF8/CFC are the architected PCI configuration ports.
    unsafe {
        outl(PCI_CONFIG_ADDRESS, pci_address(bus, device, function, offset));
        inl(PCI_CONFIG_DATA)
    }
}

unsafe fn pci_write32(bus: u8, device: u8, function: u8, offset: u8, value: u32) {
    // SAFETY: CF8/CFC are the architected PCI configuration ports.
    unsafe {
        outl(PCI_CONFIG_ADDRESS, pci_address(bus, device, function, offset));
        outl(PCI_CONFIG_DATA, value);
    }
}

// ---- MMIO on the controller's BAR0 register block -----------------------

unsafe fn mmio_read32(base: u64, offset: u64) -> u32 {
    // SAFETY: `base+offset` is inside the identity-mapped BAR0 MMIO window.
    unsafe { ((base + offset) as *const u32).read_volatile() }
}

unsafe fn mmio_write32(base: u64, offset: u64, value: u32) {
    // SAFETY: `base+offset` is inside the identity-mapped BAR0 MMIO window.
    unsafe { ((base + offset) as *mut u32).write_volatile(value) };
}

// ---- Controller register offsets (NVMe base spec) -----------------------

const REG_CAP: u64 = 0x00; // capabilities (64-bit)
const REG_CC: u64 = 0x14; // controller configuration
const REG_CSTS: u64 = 0x1c; // controller status
const REG_AQA: u64 = 0x24; // admin queue attributes
const REG_ASQ: u64 = 0x28; // admin submission queue base (64-bit)
const REG_ACQ: u64 = 0x30; // admin completion queue base (64-bit)
const DOORBELL_BASE: u64 = 0x1000;

const CC_EN: u32 = 1 << 0;
const CSTS_RDY: u32 = 1 << 0;
const CSTS_CFS: u32 = 1 << 1; // controller fatal status

/// CC with the NVM command set, 4 KiB pages, 64-byte SQ / 16-byte CQ entries,
/// and the enable bit: IOSQES=6, IOCQES=4, CSS=0, MPS=0, EN=1.
const CC_ENABLE: u32 = (4 << 20) | (6 << 16) | CC_EN;

const OPCODE_IDENTIFY: u8 = 0x06;
const IDENTIFY_CNS_CONTROLLER: u32 = 1;
/// Command identifier for our one admin command; echoed in the completion.
const IDENTIFY_CID: u16 = 1;

/// Admin queue depth (entries). Small: the proof issues one command.
const QUEUE_DEPTH: u32 = 64;

/// The bring-up map identity-covers the low 4 GiB; a BAR at or above this must be
/// mapped into the page tables before it can be touched.
const IDENTITY_LIMIT: u64 = 4 * 1024 * 1024 * 1024;
/// Pages of the BAR0 register block to map: the controller registers plus the
/// doorbell region above them.
const MMIO_PAGES: u64 = 8;
const PAGE_SIZE: u64 = 4096;

// ---- DMA structures, page-aligned identity-mapped statics ---------------

/// One 4 KiB page. The admin SQ (64 * 64 = 4096 bytes) exactly fills one; the
/// admin CQ (64 * 16) and the IDENTIFY result each need one aligned page.
#[repr(C, align(4096))]
struct Page([u8; 4096]);

static mut ADMIN_SQ: Page = Page([0; 4096]);
static mut ADMIN_CQ: Page = Page([0; 4096]);
static mut IDENTIFY: Page = Page([0; 4096]);

#[derive(Clone, Copy)]
struct PciLocation {
    bus: u8,
    device: u8,
    function: u8,
}

/// Is the device at this location an NVMe controller (class 01h/08h, prog-IF 02h)?
fn is_nvme(location: PciLocation) -> bool {
    // SAFETY: configuration reads have no side effects.
    let id = unsafe { pci_read32(location.bus, location.device, location.function, 0x00) };
    if id & 0xffff == 0xffff {
        return false;
    }
    let class = unsafe { pci_read32(location.bus, location.device, location.function, 0x08) };
    (class >> 24) & 0xff == 0x01 && (class >> 16) & 0xff == 0x08 && (class >> 8) & 0xff == 0x02
}

/// A brought-up NVMe controller: its BAR0 base and doorbell stride in bytes.
struct Controller {
    base: u64,
    doorbell_stride: u64,
    depth: u32,
}

/// Enable memory space + bus master and return the controller's 64-bit BAR0 base.
fn map_bar0(location: PciLocation) -> Option<u64> {
    let PciLocation {
        bus,
        device,
        function,
    } = location;
    // SAFETY: enable MMIO + bus mastering, then read BAR0 (a 64-bit memory BAR).
    let base = unsafe {
        let command = pci_read32(bus, device, function, 0x04);
        pci_write32(bus, device, function, 0x04, command | 0b110);
        let low = pci_read32(bus, device, function, 0x10);
        let high = pci_read32(bus, device, function, 0x14);
        (u64::from(low & 0xffff_fff0)) | (u64::from(high) << 32)
    };
    if base == 0 {
        return None;
    }
    // OVMF places 64-bit BARs above the 4 GiB identity window; map the register
    // pages (uncached, non-executable) at their physical address before touching
    // them. A BAR that already falls inside the identity window is used as is.
    if base >= IDENTITY_LIMIT {
        let flags = PageTableFlags::WRITABLE
            .union(PageTableFlags::CACHE_DISABLE)
            .union(PageTableFlags::NO_EXECUTE);
        for page in 0..MMIO_PAGES {
            let addr = base + page * PAGE_SIZE;
            // SAFETY: identity-map (virt == phys) one device-MMIO page; the frame
            // is the controller's own BAR, owned by this driver while it runs.
            if unsafe { map_page(addr, addr, flags) }.is_err() {
                return None;
            }
        }
    }
    Some(base)
}

/// Reset, configure the admin queue pair, and enable the controller.
fn bring_up(base: u64) -> Option<Controller> {
    // SAFETY: BAR0 is the controller's identity-mapped MMIO window.
    let (cap_low, cap_high) = unsafe {
        (
            mmio_read32(base, REG_CAP),
            mmio_read32(base, REG_CAP + 4),
        )
    };
    let mqes = cap_low & 0xffff; // maximum queue entries, zero-based
    let doorbell_stride = 4u64 << (cap_high & 0xf); // CAP.DSTRD
    let depth = if mqes + 1 < QUEUE_DEPTH {
        mqes + 1
    } else {
        QUEUE_DEPTH
    };
    if depth < 2 {
        return None;
    }

    let sq_phys = core::ptr::addr_of!(ADMIN_SQ) as u64;
    let cq_phys = core::ptr::addr_of!(ADMIN_CQ) as u64;

    // SAFETY: MMIO on a controller that exists; the queue bases are page-aligned
    // statics the identity map covers.
    unsafe {
        // Disable, then wait for the controller to report not-ready.
        mmio_write32(base, REG_CC, 0);
        let mut budget = 10_000_000u32;
        while mmio_read32(base, REG_CSTS) & CSTS_RDY != 0 {
            budget -= 1;
            if budget == 0 {
                return None;
            }
            core::hint::spin_loop();
        }

        // Admin queue attributes: submission and completion sizes (zero-based).
        mmio_write32(base, REG_AQA, ((depth - 1) << 16) | (depth - 1));
        mmio_write32(base, REG_ASQ, sq_phys as u32);
        mmio_write32(base, REG_ASQ + 4, (sq_phys >> 32) as u32);
        mmio_write32(base, REG_ACQ, cq_phys as u32);
        mmio_write32(base, REG_ACQ + 4, (cq_phys >> 32) as u32);

        // Enable, then wait for ready (or a fatal status).
        mmio_write32(base, REG_CC, CC_ENABLE);
        let mut budget = 10_000_000u32;
        loop {
            let csts = mmio_read32(base, REG_CSTS);
            if csts & CSTS_CFS != 0 {
                return None;
            }
            if csts & CSTS_RDY != 0 {
                break;
            }
            budget -= 1;
            if budget == 0 {
                return None;
            }
            core::hint::spin_loop();
        }
    }

    Some(Controller {
        base,
        doorbell_stride,
        depth,
    })
}

impl Controller {
    /// Issue IDENTIFY CONTROLLER into the `IDENTIFY` page and await its
    /// completion, returning the 15-bit status field (0 on success).
    ///
    /// # Safety
    /// CPL0; the controller is enabled and the DMA statics are identity-mapped.
    unsafe fn identify_controller(&self) -> Result<u16, &'static str> {
        let sq = core::ptr::addr_of_mut!(ADMIN_SQ) as *mut u32;
        let cq = core::ptr::addr_of!(ADMIN_CQ) as *const u32;
        let prp1 = core::ptr::addr_of!(IDENTIFY) as u64;

        // SAFETY: build the 64-byte submission entry at slot 0.
        unsafe {
            for dword in 0..16 {
                sq.add(dword).write_volatile(0);
            }
            sq.add(0)
                .write_volatile(u32::from(OPCODE_IDENTIFY) | (u32::from(IDENTIFY_CID) << 16)); // CDW0
            sq.add(1).write_volatile(0); // NSID: none for identify controller
            sq.add(6).write_volatile(prp1 as u32); // PRP1 low
            sq.add(7).write_volatile((prp1 >> 32) as u32); // PRP1 high
            sq.add(10).write_volatile(IDENTIFY_CNS_CONTROLLER); // CDW10: CNS
        }

        // SAFETY: publish tail=1 to the admin submission queue doorbell.
        unsafe {
            core::arch::asm!("mfence", options(nostack, preserves_flags));
            mmio_write32(self.base, DOORBELL_BASE, 1);
        }

        // Poll completion-queue entry 0 for the phase tag (the CQ was zeroed, so a
        // fresh completion flips it to 1). CQE dword 3: [15:0] CID, [16] phase,
        // [31:17] status.
        let mut budget = 200_000_000u32;
        let dword3 = loop {
            // SAFETY: reading CQE[0] dword 3 from the identity-mapped CQ.
            let dword3 = unsafe { cq.add(3).read_volatile() };
            if dword3 & (1 << 16) != 0 {
                break dword3;
            }
            budget -= 1;
            if budget == 0 {
                return Err("no_completion");
            }
            core::hint::spin_loop();
        };

        // Advance the CQ head past the entry we consumed.
        // SAFETY: ring the admin completion-queue head doorbell.
        unsafe { mmio_write32(self.base, DOORBELL_BASE + self.doorbell_stride, 1) };

        if dword3 & 0xffff != u32::from(IDENTIFY_CID) {
            return Err("wrong_cid");
        }
        Ok(((dword3 >> 17) & 0x7fff) as u16)
    }
}

/// Scan PCI for an NVMe controller and bring up the first one found.
fn init() -> Option<Controller> {
    for bus in 0u16..256 {
        for device in 0u8..32 {
            for function in 0u8..8 {
                let location = PciLocation {
                    bus: bus as u8,
                    device,
                    function,
                };
                if !is_nvme(location) {
                    continue;
                }
                debug_write("AW_NVME_FOUND bus=");
                debug_write_u64(u64::from(location.bus));
                debug_write(" device=");
                debug_write_u64(u64::from(location.device));
                debug_write(" function=");
                debug_write_u64(u64::from(location.function));
                debug_write("\n");
                let base = map_bar0(location)?;
                debug_write("AW_NVME_BAR0 base=");
                debug_write_hex_u64(base);
                debug_write("\n");
                return bring_up(base);
            }
        }
    }
    debug_write("AW_NVME_UNAVAILABLE reason=no_controller\n");
    None
}

/// Print an ASCII field with trailing spaces and NULs trimmed, and report whether
/// it was non-empty and entirely printable ASCII.
fn write_ascii_field(field: &[u8]) -> bool {
    let mut end = field.len();
    while end > 0 && (field[end - 1] == b' ' || field[end - 1] == 0) {
        end -= 1;
    }
    if end == 0 {
        return false;
    }
    let mut printable = true;
    for &byte in &field[..end] {
        if !(0x20..=0x7e).contains(&byte) {
            printable = false;
            break;
        }
        let one = [byte];
        // SAFETY: a single 0x20..=0x7e byte is valid single-byte UTF-8.
        debug_write(unsafe { core::str::from_utf8_unchecked(&one) });
    }
    printable
}

/// Prove an NVMe controller bring-up: enable it, run IDENTIFY CONTROLLER, and
/// read back the model number the controller wrote by DMA - real content, not a
/// status bit. Prints `AW_NVME_UNAVAILABLE` and returns if no controller is
/// present, so it is safe to call on every boot configuration.
pub fn prove() {
    debug_write("AW_NVME_BEGIN\n");
    // init() prints its own AW_NVME_UNAVAILABLE reason when nothing usable is found.
    let Some(controller) = init() else {
        return;
    };
    debug_write("AW_NVME_ENABLED depth=");
    debug_write_u64(u64::from(controller.depth));
    debug_write("\n");

    // SAFETY: CPL0; the controller is enabled and the DMA statics are live.
    let status = match unsafe { controller.identify_controller() } {
        Ok(status) => status,
        Err(reason) => {
            debug_write("AW_NVME_FAIL reason=");
            debug_write(reason);
            debug_write("\n");
            return;
        }
    };
    if status != 0 {
        debug_write("AW_NVME_FAIL reason=status sc=");
        debug_write_hex_u64(u64::from(status));
        debug_write("\n");
        return;
    }

    // Identify Controller structure: model number is 40 bytes at offset 24.
    let identify = core::ptr::addr_of!(IDENTIFY) as *const u8;
    let mut model = [0u8; 40];
    // SAFETY: the controller wrote a full 4 KiB page into the IDENTIFY static.
    for (index, slot) in model.iter_mut().enumerate() {
        *slot = unsafe { identify.add(24 + index).read_volatile() };
    }

    debug_write("AW_NVME_IDENTIFY_OK model=");
    let printable = write_ascii_field(&model);
    debug_write("\n");
    if printable {
        debug_write("AW_NVME_PROOF_OK\n");
    } else {
        debug_write("AW_NVME_FAIL reason=empty_model\n");
    }
}
