#![no_main]
#![no_std]

use uefi::prelude::*;

#[entry]
fn main() -> Status {
    if uefi::helpers::init().is_err() {
        return Status::ABORTED;
    }

    log::info!("AW_BOOT_OK stage=uefi_init arch=x86_64");

    uefi::println!("Accessible Windows");
    uefi::println!("BOOT_STAGE=UEFI_INIT");
    uefi::println!("ARCH=x86_64");

    Status::SUCCESS
}
