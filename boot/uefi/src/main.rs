#![no_main]
#![no_std]

extern crate alloc;

use alloc::vec;
use aw_acpi::{RsdpError, RsdpInfo};
use aw_kernel_core::{FramebufferHandoff, HandoffPixelFormat, KernelHandoff};
use uefi::boot::{self, AllocateType};
use uefi::mem::memory_map::{MemoryMap, MemoryType};
use uefi::prelude::*;
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat as UefiPixelFormat};
use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileMode};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::table::cfg::ConfigTableEntry;
use uefi::{cstr16, system, Status};

const KERNEL_LOAD_ADDRESS: u64 = 0x0100_0000;
const UEFI_PAGE_SIZE: usize = 4096;

fn validate_firmware_rsdp(address: usize, table_revision: u8) -> Result<RsdpInfo, RsdpError> {
    let probe_len = if table_revision >= 2 {
        aw_acpi::RSDP_V2_MIN_LEN
    } else {
        aw_acpi::RSDP_V1_LEN
    };

    // SAFETY: `address` comes directly from the UEFI ACPI configuration-table
    // entry selected by its ACPI GUID. UEFI guarantees that this entry points
    // to an RSDP structure. We only borrow the minimum structure size here and
    // perform this validation before ExitBootServices.
    let probe = unsafe { core::slice::from_raw_parts(address as *const u8, probe_len) };
    let declared_length = aw_acpi::declared_length(probe)?;

    if declared_length <= probe.len() {
        return aw_acpi::validate_rsdp(&probe[..declared_length]);
    }

    // SAFETY: The validated RSDP prefix declares its own total length. The
    // parser caps that value at `RSDP_MAX_LEN`; UEFI's ACPI configuration-table
    // contract identifies the pointer as the complete RSDP. This expanded
    // borrow is also consumed before ExitBootServices and is not retained.
    let full = unsafe { core::slice::from_raw_parts(address as *const u8, declared_length) };
    aw_acpi::validate_rsdp(full)
}

fn load_native_kernel() -> Result<usize, Status> {
    let mut file_system = match boot::get_image_file_system(boot::image_handle()) {
        Ok(file_system) => {
            log::info!("AW_KERNEL_FS_OK source=image_handle");
            file_system
        }
        Err(error) => {
            log::warn!(
                "AW_KERNEL_FS_FALLBACK image_status={:?}",
                error.status()
            );
            let handle = boot::get_handle_for_protocol::<SimpleFileSystem>()
                .map_err(|error| error.status())?;
            let file_system = boot::open_protocol_exclusive::<SimpleFileSystem>(handle)
                .map_err(|error| error.status())?;
            log::info!("AW_KERNEL_FS_OK source=protocol_scan");
            file_system
        }
    };

    let mut root = file_system.open_volume().map_err(|error| {
        log::error!("AW_KERNEL_VOLUME_FAIL status={:?}", error.status());
        error.status()
    })?;

    loop {
        match root.read_entry_boxed() {
            Ok(Some(entry)) => log::info!(
                "AW_ESP_ENTRY name={:?} size={} directory={}",
                entry.file_name(),
                entry.file_size(),
                entry.is_directory()
            ),
            Ok(None) => break,
            Err(error) => {
                log::warn!("AW_ESP_ENUM_FAIL status={:?}", error.status());
                break;
            }
        }
    }

    root.reset_entry_readout().map_err(|error| error.status())?;
    let kernel_handle = root
        .open(cstr16!("KERNEL.BIN"), FileMode::Read, FileAttribute::empty())
        .map_err(|error| {
            log::error!("AW_KERNEL_OPEN_FAIL status={:?}", error.status());
            error.status()
        })?;
    let mut kernel_file = kernel_handle.into_regular_file().ok_or(Status::LOAD_ERROR)?;
    let kernel_info = kernel_file.get_boxed_info::<FileInfo>().map_err(|error| {
        log::error!("AW_KERNEL_INFO_FAIL status={:?}", error.status());
        error.status()
    })?;
    let kernel_size = usize::try_from(kernel_info.file_size()).map_err(|_| Status::BAD_BUFFER_SIZE)?;
    drop(kernel_info);

    if kernel_size == 0 {
        return Err(Status::LOAD_ERROR);
    }

    let mut kernel_image = vec![0_u8; kernel_size];
    let mut offset = 0_usize;
    while offset < kernel_image.len() {
        let read = kernel_file.read(&mut kernel_image[offset..]).map_err(|error| {
            log::error!("AW_KERNEL_READ_FAIL status={:?}", error.status());
            error.status()
        })?;
        if read == 0 {
            break;
        }
        offset += read;
    }

    if offset != kernel_image.len() {
        log::error!(
            "AW_KERNEL_READ_SHORT expected={} actual={}",
            kernel_image.len(),
            offset
        );
        return Err(Status::LOAD_ERROR);
    }

    log::info!("AW_KERNEL_FILE_READ_OK bytes={}", kernel_image.len());

    let pages = kernel_image.len().div_ceil(UEFI_PAGE_SIZE);
    let allocation = boot::allocate_pages(
        AllocateType::Address(KERNEL_LOAD_ADDRESS),
        MemoryType::LOADER_DATA,
        pages,
    )
    .map_err(|error| error.status())?;

    if allocation.as_ptr() as u64 != KERNEL_LOAD_ADDRESS {
        return Err(Status::LOAD_ERROR);
    }

    let allocation_len = pages * UEFI_PAGE_SIZE;
    // SAFETY: `allocation` owns `allocation_len` writable bytes allocated by
    // UEFI at the exact kernel load address. The source Vec is valid and the
    // copy length is bounded by the allocated page count.
    unsafe {
        core::ptr::copy_nonoverlapping(
            kernel_image.as_ptr(),
            allocation.as_ptr(),
            kernel_image.len(),
        );
        if allocation_len > kernel_image.len() {
            core::ptr::write_bytes(
                allocation.as_ptr().add(kernel_image.len()),
                0,
                allocation_len - kernel_image.len(),
            );
        }
    }

    log::info!(
        "AW_NATIVE_KERNEL_LOAD_OK address=0x{:x} bytes={} pages={}",
        KERNEL_LOAD_ADDRESS,
        kernel_image.len(),
        pages
    );

    Ok(kernel_image.len())
}

#[entry]
fn main() -> Status {
    if uefi::helpers::init().is_err() {
        return Status::ABORTED;
    }

    log::info!("AW_BOOT_OK stage=uefi_init arch=x86_64");

    let kernel_size = match load_native_kernel() {
        Ok(size) => size,
        Err(status) => {
            log::error!("AW_NATIVE_KERNEL_LOAD_FAIL status={:?}", status);
            return status;
        }
    };

    let memory_map = match boot::memory_map(MemoryType::LOADER_DATA) {
        Ok(memory_map) => memory_map,
        Err(_) => {
            log::error!("AW_MEMORY_MAP_FAIL");
            return Status::DEVICE_ERROR;
        }
    };
    log::info!("AW_MEMORY_MAP_OK entries={}", memory_map.len());
    drop(memory_map);

    let acpi = system::with_config_table(|tables| {
        tables
            .iter()
            .find(|entry| entry.guid == ConfigTableEntry::ACPI2_GUID)
            .map(|entry| (entry.address as usize, 2_u8))
            .or_else(|| {
                tables
                    .iter()
                    .find(|entry| entry.guid == ConfigTableEntry::ACPI_GUID)
                    .map(|entry| (entry.address as usize, 1_u8))
            })
    });

    let Some((acpi_address, acpi_table_revision)) = acpi else {
        log::error!("AW_ACPI_FAIL reason=no_rsdp");
        return Status::NOT_FOUND;
    };
    log::info!(
        "AW_ACPI_OK table_revision={} rsdp=0x{:x}",
        acpi_table_revision,
        acpi_address
    );

    let rsdp = match validate_firmware_rsdp(acpi_address, acpi_table_revision) {
        Ok(rsdp) => rsdp,
        Err(error) => {
            log::error!("AW_ACPI_VALIDATE_FAIL error={:?}", error);
            return Status::COMPROMISED_DATA;
        }
    };
    log::info!(
        "AW_ACPI_VALIDATE_OK revision={} length={} rsdt=0x{:x} xsdt=0x{:x}",
        rsdp.revision,
        rsdp.length,
        rsdp.rsdt_address,
        rsdp.xsdt_address.unwrap_or(0)
    );

    let gop_handle = match boot::get_handle_for_protocol::<GraphicsOutput>() {
        Ok(handle) => handle,
        Err(_) => {
            log::error!("AW_GOP_FAIL reason=no_handle");
            return Status::NOT_FOUND;
        }
    };

    let mut gop = match boot::open_protocol_exclusive::<GraphicsOutput>(gop_handle) {
        Ok(gop) => gop,
        Err(_) => {
            log::error!("AW_GOP_FAIL reason=open_protocol");
            return Status::DEVICE_ERROR;
        }
    };

    let mode = gop.current_mode_info();
    let (width, height) = mode.resolution();
    let stride_pixels = mode.stride();
    let uefi_pixel_format = mode.pixel_format();
    let handoff_pixel_format = match uefi_pixel_format {
        UefiPixelFormat::Rgb => HandoffPixelFormat::Rgb,
        UefiPixelFormat::Bgr => HandoffPixelFormat::Bgr,
        UefiPixelFormat::Bitmask => HandoffPixelFormat::Bitmask,
        UefiPixelFormat::BltOnly => HandoffPixelFormat::Unknown,
    };

    log::info!(
        "AW_GOP_OK width={} height={} stride={} format={:?}",
        width,
        height,
        stride_pixels,
        uefi_pixel_format
    );

    let framebuffer = if uefi_pixel_format == UefiPixelFormat::BltOnly {
        log::warn!("AW_FRAMEBUFFER_UNAVAILABLE reason=blt_only");
        None
    } else {
        let mut frame_buffer = gop.frame_buffer();
        let address = frame_buffer.as_mut_ptr() as usize;
        let size = frame_buffer.size();
        log::info!("AW_FRAMEBUFFER_OK address=0x{:x} size={}", address, size);
        Some(FramebufferHandoff {
            physical_address: address as u64,
            byte_len: size as u64,
            width: width as u32,
            height: height as u32,
            stride_pixels: stride_pixels as u32,
            pixel_format: handoff_pixel_format,
        })
    };

    uefi::println!("Accessible Windows");
    uefi::println!("BOOT_STAGE=UEFI_HARDWARE_DISCOVERY");
    uefi::println!("ARCH=x86_64");
    uefi::println!("DISPLAY={}x{}", width, height);

    drop(gop);

    log::info!("AW_EXIT_BOOT_SERVICES_BEGIN");

    // SAFETY: All boot-services-backed protocol objects and temporary memory
    // maps have been dropped. The kernel image lives in LOADER_DATA pages and
    // remains valid after ExitBootServices.
    let final_memory_map = unsafe { boot::exit_boot_services(None) };

    log::info!(
        "AW_EXIT_BOOT_SERVICES_OK entries={}",
        final_memory_map.len()
    );

    let handoff = KernelHandoff::new(
        acpi_address as u64,
        final_memory_map.len() as u64,
        framebuffer,
    );

    if let Err(error) = aw_kernel_core::enter(&handoff) {
        log::error!("AW_KERNEL_HANDOFF_FAIL error={:?}", error);
        loop {
            core::hint::spin_loop();
        }
    }

    log::info!(
        "AW_KERNEL_HANDOFF_OK magic=0x{:x} abi={} size={} memory_entries={} flags=0x{:x}",
        handoff.magic,
        handoff.abi_version,
        handoff.struct_size,
        handoff.memory_map_entries,
        handoff.flags
    );
    log::info!(
        "AW_NATIVE_KERNEL_TRANSFER address=0x{:x} bytes={}",
        KERNEL_LOAD_ADDRESS,
        kernel_size
    );

    type KernelEntry = extern "sysv64" fn(*const KernelHandoff) -> !;
    // SAFETY: The raw kernel image was linked for KERNEL_LOAD_ADDRESS and copied
    // there before ExitBootServices. Its first linked symbol is `_start` using
    // the SysV64 ABI defined by this loader/kernel contract.
    let kernel_entry: KernelEntry = unsafe { core::mem::transmute(KERNEL_LOAD_ADDRESS as usize) };
    kernel_entry(core::ptr::addr_of!(handoff));
}
