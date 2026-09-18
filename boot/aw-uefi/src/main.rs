#![no_std]
#![no_main]
//! First UEFI entry point for the new OS branch.

use aw_abi::{BootInfo, BootPhase};
use aw_accessibility::AccessibilityState;
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
/// The first milestone deliberately assumes no accessibility capability. Future
/// firmware adapters must positively observe each capability before handoff.
#[unsafe(no_mangle)]
pub extern "efiapi" fn efi_main(_image_handle: *mut c_void, _system_table: *mut c_void) -> usize {
    let state = AccessibilityState::new(BootPhase::Bootloader);
    let _boot_info = BootInfo::new(BootPhase::Bootloader, state.strict_contract());

    EFI_SUCCESS
}
