//! Minimal GPT partition-table reader (dossier section 12, roadmap Phase 3
//! "GPT parser/writer with safety checks").
//!
//! Reading a disk's own partition table is the first step of recognising and later
//! installing to a real disk. This reads the GPT header at LBA 1 off an AHCI disk,
//! checks the `EFI PART` signature and the header's own CRC32, then walks the first
//! sector of the partition entry array and reports the first real partition - the
//! EFI System Partition on a normal disk. It is read-only and safe on any disk;
//! a disk with no GPT (a bare FAT image, say) is reported unavailable, not failed.

use crate::ahci::{self, SECTOR_SIZE};
use crate::{debug_write, debug_write_hex_u64, debug_write_u64};

fn read_u32(buffer: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        buffer[offset],
        buffer[offset + 1],
        buffer[offset + 2],
        buffer[offset + 3],
    ])
}

fn read_u64(buffer: &[u8], offset: usize) -> u64 {
    u64::from(read_u32(buffer, offset)) | (u64::from(read_u32(buffer, offset + 4)) << 32)
}

/// The EFI System Partition type GUID (C12A7328-F81F-11D2-BA4B-00A0C93EC93B), in
/// on-disk mixed-endian byte order.
const ESP_TYPE_GUID: [u8; 16] = [
    0x28, 0x73, 0x2a, 0xc1, 0x1f, 0xf8, 0xd2, 0x11, 0xba, 0x4b, 0x00, 0xa0, 0xc9, 0x3e, 0xc9, 0x3b,
];

/// CRC32 (IEEE, reflected) of `data`, as GPT uses for its header check.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn guid_is_zero(guid: &[u8]) -> bool {
    guid.iter().all(|&b| b == 0)
}

/// Read and validate the GPT off the first AHCI disk, and report its first
/// partition. Prints `AW_GPT_UNAVAILABLE` and returns when there is no AHCI disk
/// or the disk carries no GPT, so it is safe on every boot configuration.
pub fn prove() {
    debug_write("AW_GPT_BEGIN\n");
    let Some(port) = ahci::init() else {
        debug_write("AW_GPT_UNAVAILABLE reason=no_disk\n");
        return;
    };

    let mut header = [0u8; SECTOR_SIZE];
    if port.read_sector(1, &mut header).is_err() {
        debug_write("AW_GPT_UNAVAILABLE reason=read_header\n");
        return;
    }
    if &header[0..8] != b"EFI PART" {
        debug_write("AW_GPT_UNAVAILABLE reason=no_gpt\n");
        return;
    }

    // Verify the header CRC32: the field is computed with its own four bytes
    // zeroed, over header_size bytes (bytes 12..16).
    let header_size = read_u32(&header, 12) as usize;
    let stored_crc = read_u32(&header, 16);
    if !(92..=SECTOR_SIZE).contains(&header_size) {
        debug_write("AW_GPT_FAIL reason=bad_header_size\n");
        return;
    }
    let mut scratch = header;
    scratch[16] = 0;
    scratch[17] = 0;
    scratch[18] = 0;
    scratch[19] = 0;
    let computed = crc32(&scratch[0..header_size]);
    if computed != stored_crc {
        debug_write("AW_GPT_FAIL reason=header_crc stored=");
        debug_write_hex_u64(u64::from(stored_crc));
        debug_write(" computed=");
        debug_write_hex_u64(u64::from(computed));
        debug_write("\n");
        return;
    }
    debug_write("AW_GPT_HEADER_OK\n");

    let entries_lba = read_u64(&header, 72);
    let entry_count = read_u32(&header, 80);
    let entry_size = read_u32(&header, 84) as usize;
    debug_write("AW_GPT_TABLE entries=");
    debug_write_u64(u64::from(entry_count));
    debug_write(" entry_size=");
    debug_write_u64(entry_size as u64);
    debug_write("\n");
    if !(128..=SECTOR_SIZE).contains(&entry_size) {
        debug_write("AW_GPT_FAIL reason=bad_entry_size\n");
        return;
    }

    // The first sector of the entry array is enough to find the first partition.
    let mut table = [0u8; SECTOR_SIZE];
    if port.read_sector(entries_lba, &mut table).is_err() {
        debug_write("AW_GPT_FAIL reason=read_entries\n");
        return;
    }

    let per_sector = SECTOR_SIZE / entry_size;
    for index in 0..per_sector.min(entry_count as usize) {
        let base = index * entry_size;
        let type_guid = &table[base..base + 16];
        if guid_is_zero(type_guid) {
            continue; // unused entry
        }
        let first_lba = read_u64(&table, base + 32);
        let last_lba = read_u64(&table, base + 40);
        let is_esp = type_guid == ESP_TYPE_GUID;
        debug_write("AW_GPT_PARTITION index=");
        debug_write_u64(index as u64);
        debug_write(" first_lba=");
        debug_write_u64(first_lba);
        debug_write(" last_lba=");
        debug_write_u64(last_lba);
        debug_write(if is_esp { " esp=1\n" } else { " esp=0\n" });
        debug_write("AW_GPT_PROOF_OK\n");
        return;
    }

    debug_write("AW_GPT_FAIL reason=no_partition\n");
}
