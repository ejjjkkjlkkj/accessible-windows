#![no_main]
#![no_std]

use aw_acpi::{RsdpError, RsdpInfo};
use uefi::mem::memory_map::{MemoryMap, MemoryType};
use uefi::prelude::*;
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat};
use uefi::table::cfg::ConfigTableEntry;
use uefi::{boot, system, Status};

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

#[entry]
fn main() -> Status {
    if uefi::helpers::init().is_err() {
        return Status::ABORTED;
    }

    log::info!("AW_BOOT_OK stage=uefi_init arch=x86_64");

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
    log::info!(
        "AW_GOP_OK width={} height={} stride={} format={:?}",
        width,
        height,
        mode.stride(),
        mode.pixel_format()
    );

    let framebuffer = if mode.pixel_format() == PixelFormat::BltOnly {
        log::warn!("AW_FRAMEBUFFER_UNAVAILABLE reason=blt_only");
        None
    } else {
        let mut frame_buffer = gop.frame_buffer();
        let address = frame_buffer.as_mut_ptr() as usize;
        let size = frame_buffer.size();
        log::info!("AW_FRAMEBUFFER_OK address=0x{:x} size={}", address, size);
        Some((address, size))
    };

    uefi::println!("Accessible Windows");
    uefi::println!("BOOT_STAGE=UEFI_HARDWARE_DISCOVERY");
    uefi::println!("ARCH=x86_64");
    uefi::println!("DISPLAY={}x{}", width, height);

    drop(gop);

    log::info!("AW_EXIT_BOOT_SERVICES_BEGIN");

    // SAFETY: All boot-services-backed protocol objects and temporary memory
    // maps have been dropped. The remaining handoff data is copied into scalar
    // values. After this call, this function uses no UEFI Boot Services APIs.
    let final_memory_map = unsafe { boot::exit_boot_services(None) };

    log::info!(
        "AW_EXIT_BOOT_SERVICES_OK entries={}",
        final_memory_map.len()
    );

    match framebuffer {
        Some((address, size)) => log::info!(
            "AW_KERNEL_STAGE_OK acpi_rsdp=0x{:x} framebuffer=0x{:x} framebuffer_size={}",
            acpi_address,
            address,
            size
        ),
        None => log::info!(
            "AW_KERNEL_STAGE_OK acpi_rsdp=0x{:x} framebuffer=unavailable",
            acpi_address
        ),
    }

    loop {
        core::hint::spin_loop();
    }
}
