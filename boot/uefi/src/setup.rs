//! Fully keyboard-operable firmware setup owned by Accessible Windows.
//!
//! The interface is independent from a vendor graphical BIOS UI: every page is
//! available through the UEFI text console, every focus change is spoken through
//! HDA when real speech assets are present, and dynamic values are spelled so they
//! never remain visual-only. Up/Down or Tab moves, Enter activates, Escape returns.
//! Once the user interacts the unattended timeout is permanently disabled.

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::time::Duration;

use uefi::boot;
use uefi::proto::console::text::{Key, ScanCode};
use uefi::runtime::{self, ResetType, VariableAttributes, VariableVendor};
use uefi::system;
use uefi::{cstr16, CString16, Status};

use crate::hda;
use crate::setup_speech::{self, Clip};
use crate::sound;

const IDLE_WINDOW: Duration = Duration::from_secs(3);
const POLL_INTERVAL: Duration = Duration::from_millis(50);
const CUE_MOVE_HZ: u32 = 660;
const CUE_ENTER_HZ: u32 = 784;
const CUE_BACK_HZ: u32 = 523;
const MAX_BOOT_OPTIONS: usize = 32;

include!("setup_parts/core.rs");
include!("setup_parts/ui.rs");
include!("setup_parts/actions.rs");
include!("setup_parts/run.rs");
