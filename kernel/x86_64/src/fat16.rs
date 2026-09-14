//! Read-only FAT16 file read from a virtio-block disk (dossier section 12,
//! roadmap P0 step 7).
//!
//! This is the first filesystem: it parses the FAT16 BIOS parameter block on the
//! virtio disk, walks the root directory for a known 8.3 file, follows its
//! cluster chain through the FAT, and returns the bytes - proving a real file
//! read end to end, device through filesystem. It uses `alloc` (the kernel
//! heap) for the FAT and the assembled file, so it also exercises that path.
//!
//! FAT is little-endian, matching the guest. Only what this proof needs is
//! implemented: no long names, no writing, no subdirectories.

use alloc::vec::Vec;

use crate::virtio_blk::{BlkDevice, SECTOR_SIZE};
use crate::{debug_write, debug_write_u64};

/// The file the test disk carries, as an 8.3 directory name and its contents.
const TARGET_NAME: &[u8; 11] = b"HELLO   TXT";
const EXPECTED: &[u8] = b"ACCESSIBLE-WINDOWS-FS-OK\n";

fn read_u16(buffer: &[u8], offset: usize) -> u16 {
    u16::from(buffer[offset]) | (u16::from(buffer[offset + 1]) << 8)
}

fn read_u32(buffer: &[u8], offset: usize) -> u32 {
    u32::from(buffer[offset])
        | (u32::from(buffer[offset + 1]) << 8)
        | (u32::from(buffer[offset + 2]) << 16)
        | (u32::from(buffer[offset + 3]) << 24)
}

fn slices_equal(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x == y)
}

/// Where a FAT16 volume's regions start, read from its boot sector.
struct Geometry {
    sector_size: usize,
    sectors_per_cluster: usize,
    fat_start: usize,
    root_start: usize,
    root_sectors: usize,
    data_start: usize,
    fat_sectors: usize,
}

fn parse_geometry(boot: &[u8; SECTOR_SIZE]) -> Option<Geometry> {
    let sector_size = read_u16(boot, 0x0b) as usize;
    let sectors_per_cluster = boot[0x0d] as usize;
    let reserved = read_u16(boot, 0x0e) as usize;
    let num_fats = boot[0x10] as usize;
    let root_entries = read_u16(boot, 0x11) as usize;
    let fat_sectors = read_u16(boot, 0x16) as usize;
    if sector_size != SECTOR_SIZE || sectors_per_cluster == 0 || fat_sectors == 0 || num_fats == 0 {
        return None;
    }
    let fat_start = reserved;
    let root_start = reserved + num_fats * fat_sectors;
    let root_sectors = (root_entries * 32).div_ceil(sector_size);
    let data_start = root_start + root_sectors;
    Some(Geometry {
        sector_size,
        sectors_per_cluster,
        fat_start,
        root_start,
        root_sectors,
        data_start,
        fat_sectors,
    })
}

/// Scan the root directory for the 8.3 `name`, returning (first cluster, size).
fn find_file(
    device: &BlkDevice,
    geometry: &Geometry,
    name: &[u8; 11],
) -> Result<Option<(u32, u32)>, &'static str> {
    let mut sector = [0u8; SECTOR_SIZE];
    for index in 0..geometry.root_sectors {
        device
            .read_sector((geometry.root_start + index) as u64, &mut sector)
            .map_err(|_| "read_root")?;
        let mut offset = 0;
        while offset < geometry.sector_size {
            let entry = &sector[offset..offset + 32];
            match entry[0] {
                0x00 => return Ok(None), // no further entries
                0xe5 => {}               // deleted
                _ if entry[11] == 0x0f => {} // long-name entry
                _ if slices_equal(&entry[0..11], name) => {
                    let first_cluster =
                        u32::from(read_u16(entry, 0x1a)) | (u32::from(read_u16(entry, 0x14)) << 16);
                    let size = read_u32(entry, 0x1c);
                    return Ok(Some((first_cluster, size)));
                }
                _ => {}
            }
            offset += 32;
        }
    }
    Ok(None)
}

/// Read the whole file by following its cluster chain through the FAT.
fn read_file(
    device: &BlkDevice,
    geometry: &Geometry,
    first_cluster: u32,
    size: u32,
) -> Result<Vec<u8>, &'static str> {
    // Load the FAT so the chain can be followed without re-reading sectors.
    let mut fat = Vec::new();
    let mut sector = [0u8; SECTOR_SIZE];
    for index in 0..geometry.fat_sectors {
        device
            .read_sector((geometry.fat_start + index) as u64, &mut sector)
            .map_err(|_| "read_fat")?;
        fat.extend_from_slice(&sector);
    }

    let mut contents = Vec::new();
    let mut cluster = first_cluster;
    let mut guard = 0u32;
    while (2..0xfff8).contains(&cluster) && contents.len() < size as usize {
        let first_sector = geometry.data_start + (cluster as usize - 2) * geometry.sectors_per_cluster;
        for index in 0..geometry.sectors_per_cluster {
            device
                .read_sector((first_sector + index) as u64, &mut sector)
                .map_err(|_| "read_data")?;
            contents.extend_from_slice(&sector);
        }
        let entry = (cluster as usize) * 2;
        cluster = read_u16(&fat, entry) as u32;
        guard += 1;
        if guard > 4096 {
            return Err("chain_too_long");
        }
    }
    contents.truncate(size as usize);
    Ok(contents)
}

/// Read a whole file named by its 8.3 directory name (e.g. `b"USERPROGELF"`),
/// returning its bytes, or [`None`] if the volume cannot be parsed or the file is
/// absent. The reusable entry point behind [`prove`], used by the userland loader.
pub fn load_file(device: &BlkDevice, name: &[u8; 11]) -> Option<Vec<u8>> {
    let mut boot = [0u8; SECTOR_SIZE];
    device.read_sector(0, &mut boot).ok()?;
    let geometry = parse_geometry(&boot)?;
    let (first_cluster, size) = find_file(device, &geometry, name).ok()??;
    read_file(device, &geometry, first_cluster, size).ok()
}

/// Prove a file read from a FAT16 filesystem on the virtio disk.
pub fn prove(device: &BlkDevice) {
    debug_write("AW_FS_BEGIN\n");

    let mut boot = [0u8; SECTOR_SIZE];
    if device.read_sector(0, &mut boot).is_err() {
        debug_write("AW_FS_FAIL reason=read_boot\n");
        return;
    }
    let Some(geometry) = parse_geometry(&boot) else {
        debug_write("AW_FS_FAIL reason=bad_bpb\n");
        return;
    };

    let file = match find_file(device, &geometry, TARGET_NAME) {
        Ok(Some(file)) => file,
        Ok(None) => {
            debug_write("AW_FS_FAIL reason=not_found\n");
            return;
        }
        Err(reason) => {
            debug_write("AW_FS_FAIL reason=");
            debug_write(reason);
            debug_write("\n");
            return;
        }
    };
    let (first_cluster, size) = file;
    debug_write("AW_FS_FILE_FOUND size=");
    debug_write_u64(u64::from(size));
    debug_write("\n");

    let contents = match read_file(device, &geometry, first_cluster, size) {
        Ok(contents) => contents,
        Err(reason) => {
            debug_write("AW_FS_FAIL reason=");
            debug_write(reason);
            debug_write("\n");
            return;
        }
    };

    if slices_equal(&contents, EXPECTED) {
        debug_write("AW_FS_READ_OK\n");
        debug_write("AW_FS_PROOF_OK\n");
    } else {
        debug_write("AW_FS_FAIL reason=content_mismatch\n");
    }
}
