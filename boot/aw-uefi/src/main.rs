#![no_std]
#![no_main]
//! First UEFI entry point for the new OS branch.

use aw_abi::BootPhase;
use aw_core::SystemState;
use core::{ffi::c_void, panic::PanicInfo};

const EFI_SUCCESS: usize = 0;

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

/// UEFI entry point.
///
/// No human-I/O channel is assumed. Native firmware adapters must positively
/// verify channels before a later milestone is permitted to hand off control to
/// the kernel.
#[unsafe(no_mangle)]
pub extern "efiapi" fn efi_main(_image_handle: *mut c_void, _system_table: *mut c_void) -> usize {
    let state = SystemState::new(BootPhase::Bootloader);
    let _boot_info = state.boot_info();

    EFI_SUCCESS
}
