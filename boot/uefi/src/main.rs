#![no_main]
#![no_std]

use uefi::mem::memory_map::{MemoryMap, MemoryType};
use uefi::prelude::*;
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat};
use uefi::table::cfg::ConfigTableEntry;
use uefi::{boot, system, Status};

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

    let Some((acpi_address, acpi_revision)) = acpi else {
        log::error!("AW_ACPI_FAIL reason=no_rsdp");
        return Status::NOT_FOUND;
    };
    log::info!(
        "AW_ACPI_OK revision={} rsdp=0x{:x}",
        acpi_revision,
        acpi_address
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

    if mode.pixel_format() == PixelFormat::BltOnly {
        log::warn!("AW_FRAMEBUFFER_UNAVAILABLE reason=blt_only");
    } else {
        let mut frame_buffer = gop.frame_buffer();
        let address = frame_buffer.as_mut_ptr() as usize;
        let size = frame_buffer.size();
        log::info!("AW_FRAMEBUFFER_OK address=0x{:x} size={}", address, size);
    }

    uefi::println!("Accessible Windows");
    uefi::println!("BOOT_STAGE=UEFI_HARDWARE_DISCOVERY");
    uefi::println!("ARCH=x86_64");
    uefi::println!("DISPLAY={}x{}", width, height);

    Status::SUCCESS
}
