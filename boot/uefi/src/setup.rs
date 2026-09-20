//! Hierarchical, keyboard-only and spoken UEFI Setup for Accessible Windows.
//!
//! Everything exposed here remains operable without sight. Fixed controls are
//! spoken as complete PCM phrases; firmware-provided values are spelled through
//! the embedded alphanumeric speech bank. UEFI boot options are discovered from
//! BootOrder and exposed as real selectable submenus.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::arch::x86_64::__cpuid;
use core::time::Duration;

use uefi::boot;
use uefi::proto::console::text::{Key, ScanCode};
use uefi::runtime::{self, ResetType, VariableAttributes, VariableVendor};
use uefi::system;
use uefi::{cstr16, CStr16, CString16, Status};

use crate::hda;
use crate::setup_speech::{self, Clip};
use crate::sound;

const IDLE_WINDOW: Duration = Duration::from_secs(3);
const POLL_INTERVAL: Duration = Duration::from_millis(50);
const CUE_MOVE_HZ: u32 = 660;
const CUE_ENTER_HZ: u32 = 784;
const CUE_BACK_HZ: u32 = 523;
const MAX_BOOT_OPTIONS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Page {
    Root,
    Main,
    SystemInformation,
    Advanced,
    CpuConfiguration,
    Boot,
    BootPriorities,
    BootEntry,
    Security,
    SecureBoot,
    SaveExit,
    ConfirmRestart,
    ConfirmShutdown,
}

#[derive(Clone, Copy, Debug)]
enum Action {
    Enter(Page),
    Back,
    ContinueBoot,
    Restart,
    Shutdown,
    BootNow,
    SetAsDefault,
    Noop,
}

#[derive(Clone, Copy)]
struct MenuItem {
    label: &'static str,
    clip: Clip,
    action: Action,
}

struct SetupState {
    page: Page,
    selected: usize,
    selected_boot: Option<u16>,
    boot_order: Vec<u16>,
}

const ROOT_ITEMS: &[MenuItem] = &[
    MenuItem {
        label: "Main",
        clip: Clip::Main,
        action: Action::Enter(Page::Main),
    },
    MenuItem {
        label: "Advanced",
        clip: Clip::Advanced,
        action: Action::Enter(Page::Advanced),
    },
    MenuItem {
        label: "Boot",
        clip: Clip::Boot,
        action: Action::Enter(Page::Boot),
    },
    MenuItem {
        label: "Security",
        clip: Clip::Security,
        action: Action::Enter(Page::Security),
    },
    MenuItem {
        label: "Save and Exit",
        clip: Clip::SaveExit,
        action: Action::Enter(Page::SaveExit),
    },
];

const MAIN_ITEMS: &[MenuItem] = &[
    MenuItem {
        label: "System information",
        clip: Clip::SystemInformation,
        action: Action::Enter(Page::SystemInformation),
    },
    MenuItem {
        label: "CPU model",
        clip: Clip::CpuModel,
        action: Action::Noop,
    },
    MenuItem {
        label: "Firmware time",
        clip: Clip::FirmwareTime,
        action: Action::Noop,
    },
    MenuItem {
        label: "Back",
        clip: Clip::Back,
        action: Action::Back,
    },
];

const SYSTEM_INFORMATION_ITEMS: &[MenuItem] = &[
    MenuItem {
        label: "Firmware vendor",
        clip: Clip::FirmwareVendor,
        action: Action::Noop,
    },
    MenuItem {
        label: "Firmware revision",
        clip: Clip::FirmwareRevision,
        action: Action::Noop,
    },
    MenuItem {
        label: "UEFI specification revision",
        clip: Clip::UefiRevision,
        action: Action::Noop,
    },
    MenuItem {
        label: "Display resolution",
        clip: Clip::DisplayResolution,
        action: Action::Noop,
    },
    MenuItem {
        label: "Current boot option",
        clip: Clip::BootCurrent,
        action: Action::Noop,
    },
    MenuItem {
        label: "Back",
        clip: Clip::Back,
        action: Action::Back,
    },
];

const ADVANCED_ITEMS: &[MenuItem] = &[
    MenuItem {
        label: "CPU configuration",
        clip: Clip::CpuConfiguration,
        action: Action::Enter(Page::CpuConfiguration),
    },
    MenuItem {
        label: "Back",
        clip: Clip::Back,
        action: Action::Back,
    },
];

const CPU_CONFIGURATION_ITEMS: &[MenuItem] = &[
    MenuItem {
        label: "Architecture x86 64",
        clip: Clip::Architecture,
        action: Action::Noop,
    },
    MenuItem {
        label: "Virtualization capability",
        clip: Clip::VirtualizationCapability,
        action: Action::Noop,
    },
    MenuItem {
        label: "Back",
        clip: Clip::Back,
        action: Action::Back,
    },
];

const BOOT_ITEMS: &[MenuItem] = &[
    MenuItem {
        label: "Boot option priorities",
        clip: Clip::BootOptionPriorities,
        action: Action::Enter(Page::BootPriorities),
    },
    MenuItem {
        label: "Boot normally",
        clip: Clip::BootNormally,
        action: Action::ContinueBoot,
    },
    MenuItem {
        label: "Back",
        clip: Clip::Back,
        action: Action::Back,
    },
];

const BOOT_ENTRY_ITEMS: &[MenuItem] = &[
    MenuItem {
        label: "Boot now",
        clip: Clip::BootNow,
        action: Action::BootNow,
    },
    MenuItem {
        label: "Set as default",
        clip: Clip::SetAsDefault,
        action: Action::SetAsDefault,
    },
    MenuItem {
        label: "Back",
        clip: Clip::Back,
        action: Action::Back,
    },
];

const SECURITY_ITEMS: &[MenuItem] = &[
    MenuItem {
        label: "Secure Boot",
        clip: Clip::SecureBoot,
        action: Action::Enter(Page::SecureBoot),
    },
    MenuItem {
        label: "Back",
        clip: Clip::Back,
        action: Action::Back,
    },
];

const SECURE_BOOT_ITEMS: &[MenuItem] = &[
    MenuItem {
        label: "Secure Boot status",
        clip: Clip::SecureBootStatus,
        action: Action::Noop,
    },
    MenuItem {
        label: "Setup mode",
        clip: Clip::SetupMode,
        action: Action::Noop,
    },
    MenuItem {
        label: "Back",
        clip: Clip::Back,
        action: Action::Back,
    },
];

const SAVE_EXIT_ITEMS: &[MenuItem] = &[
    MenuItem {
        label: "Boot normally",
        clip: Clip::BootNormally,
        action: Action::ContinueBoot,
    },
    MenuItem {
        label: "Restart",
        clip: Clip::ResetSystem,
        action: Action::Enter(Page::ConfirmRestart),
    },
    MenuItem {
        label: "Shut down",
        clip: Clip::ShutDown,
        action: Action::Enter(Page::ConfirmShutdown),
    },
    MenuItem {
        label: "Back",
        clip: Clip::Back,
        action: Action::Back,
    },
];

const CONFIRM_RESTART_ITEMS: &[MenuItem] = &[
    MenuItem {
        label: "Confirm restart",
        clip: Clip::ConfirmRestart,
        action: Action::Restart,
    },
    MenuItem {
        label: "Cancel",
        clip: Clip::Cancel,
        action: Action::Back,
    },
];

const CONFIRM_SHUTDOWN_ITEMS: &[MenuItem] = &[
    MenuItem {
        label: "Confirm shut down",
        clip: Clip::ConfirmShutdown,
        action: Action::Shutdown,
    },
    MenuItem {
        label: "Cancel",
        clip: Clip::Cancel,
        action: Action::Back,
    },
];

fn static_items(page: Page) -> &'static [MenuItem] {
    match page {
        Page::Root => ROOT_ITEMS,
        Page::Main => MAIN_ITEMS,
        Page::SystemInformation => SYSTEM_INFORMATION_ITEMS,
        Page::Advanced => ADVANCED_ITEMS,
        Page::CpuConfiguration => CPU_CONFIGURATION_ITEMS,
        Page::Boot => BOOT_ITEMS,
        Page::BootPriorities => &[],
        Page::BootEntry => BOOT_ENTRY_ITEMS,
        Page::Security => SECURITY_ITEMS,
        Page::SecureBoot => SECURE_BOOT_ITEMS,
        Page::SaveExit => SAVE_EXIT_ITEMS,
        Page::ConfirmRestart => CONFIRM_RESTART_ITEMS,
        Page::ConfirmShutdown => CONFIRM_SHUTDOWN_ITEMS,
    }
}

fn title(page: Page) -> &'static str {
    match page {
        Page::Root => "Accessible Windows firmware setup",
        Page::Main => "Main",
        Page::SystemInformation => "System information",
        Page::Advanced => "Advanced",
        Page::CpuConfiguration => "CPU configuration",
        Page::Boot => "Boot",
        Page::BootPriorities => "Boot option priorities",
        Page::BootEntry => "Boot option",
        Page::Security => "Security",
        Page::SecureBoot => "Secure Boot",
        Page::SaveExit => "Save and Exit",
        Page::ConfirmRestart => "Confirm restart",
        Page::ConfirmShutdown => "Confirm shut down",
    }
}

fn parent(page: Page) -> Page {
    match page {
        Page::Root => Page::Root,
        Page::Main | Page::Advanced | Page::Boot | Page::Security | Page::SaveExit => Page::Root,
        Page::SystemInformation => Page::Main,
        Page::CpuConfiguration => Page::Advanced,
        Page::BootPriorities => Page::Boot,
        Page::BootEntry => Page::BootPriorities,
        Page::SecureBoot => Page::Security,
        Page::ConfirmRestart | Page::ConfirmShutdown => Page::SaveExit,
    }
}

fn poll_key() -> Option<Key> {
    system::with_stdin(|stdin| stdin.read_key().unwrap_or(None))
}

fn read_global(name: &CStr16) -> Option<(Vec<u8>, VariableAttributes)> {
    runtime::get_variable_boxed(name, &VariableVendor::GLOBAL_VARIABLE)
        .ok()
        .map(|(data, attributes)| (data.into_vec(), attributes))
}

fn read_u16(name: &CStr16) -> Option<u16> {
    let (data, _) = read_global(name)?;
    let bytes: [u8; 2] = data.get(..2)?.try_into().ok()?;
    Some(u16::from_le_bytes(bytes))
}

fn read_flag(name: &CStr16) -> Option<bool> {
    let (data, _) = read_global(name)?;
    data.first().map(|value| *value != 0)
}

fn load_boot_order() -> Vec<u16> {
    let Some((data, _)) = read_global(cstr16!("BootOrder")) else {
        return Vec::new();
    };
    data.chunks_exact(2)
        .take(MAX_BOOT_OPTIONS)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect()
}

fn boot_variable_name(id: u16) -> Option<CString16> {
    let name = format!("Boot{id:04X}");
    CString16::try_from(name.as_str()).ok()
}

fn boot_description(id: u16) -> String {
    let Some(name) = boot_variable_name(id) else {
        return format!("Boot{id:04X}");
    };
    let Some((data, _)) = read_global(&name) else {
        return format!("Boot{id:04X}");
    };
    if data.len() < 8 {
        return format!("Boot{id:04X}");
    }

    let mut description = Vec::new();
    for pair in data[6..].chunks_exact(2) {
        let unit = u16::from_le_bytes([pair[0], pair[1]]);
        if unit == 0 {
            break;
        }
        description.push(unit);
        if description.len() >= 96 {
            break;
        }
    }

    let text = String::from_utf16_lossy(&description);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        format!("Boot{id:04X}")
    } else {
        trimmed.to_string()
    }
}

fn cpu_model() -> String {
    let max_extended = __cpuid(0x8000_0000).eax;
    if max_extended < 0x8000_0004 {
        return "x86 64".to_string();
    }

    let mut bytes = Vec::with_capacity(48);
    for leaf in 0x8000_0002..=0x8000_0004 {
        let value = __cpuid(leaf);
        bytes.extend_from_slice(&value.eax.to_le_bytes());
        bytes.extend_from_slice(&value.ebx.to_le_bytes());
        bytes.extend_from_slice(&value.ecx.to_le_bytes());
        bytes.extend_from_slice(&value.edx.to_le_bytes());
    }

    core::str::from_utf8(&bytes)
        .ok()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or("x86 64")
        .to_string()
}

fn virtualization_supported() -> bool {
    let max_basic = __cpuid(0).eax;
    let vmx = max_basic >= 1 && (__cpuid(1).ecx & (1 << 5)) != 0;
    let max_extended = __cpuid(0x8000_0000).eax;
    let svm = max_extended >= 0x8000_0001 && (__cpuid(0x8000_0001).ecx & (1 << 2)) != 0;
    vmx || svm
}

fn set_boot_next(id: u16) -> bool {
    let attributes = VariableAttributes::NON_VOLATILE
        | VariableAttributes::BOOTSERVICE_ACCESS
        | VariableAttributes::RUNTIME_ACCESS;
    runtime::set_variable(
        cstr16!("BootNext"),
        &VariableVendor::GLOBAL_VARIABLE,
        attributes,
        &id.to_le_bytes(),
    )
    .is_ok()
}

fn set_boot_default(id: u16, state: &mut SetupState) -> bool {
    let Some((data, attributes)) = read_global(cstr16!("BootOrder")) else {
        return false;
    };

    let mut order: Vec<u16> = data
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();
    if !order.contains(&id) {
        return false;
    }

    order.retain(|value| *value != id);
    order.insert(0, id);

    let mut encoded = Vec::with_capacity(order.len() * 2);
    for value in &order {
        encoded.extend_from_slice(&value.to_le_bytes());
    }

    if runtime::set_variable(
        cstr16!("BootOrder"),
        &VariableVendor::GLOBAL_VARIABLE,
        attributes,
        &encoded,
    )
    .is_err()
    {
        return false;
    }

    state.boot_order = order;
    true
}

fn item_count(state: &SetupState) -> usize {
    if state.page == Page::BootPriorities {
        state.boot_order.len() + 1
    } else {
        static_items(state.page).len()
    }
}

fn render(state: &SetupState, width: usize, height: usize) {
    let _ = system::with_stdout(|stdout| stdout.clear());
    uefi::println!("{}", title(state.page));
    uefi::println!();
    uefi::println!("  Up/Down or Tab: move. Enter: open/activate. Escape: back.");
    uefi::println!();

    if state.page == Page::BootPriorities {
        for (index, id) in state.boot_order.iter().copied().enumerate() {
            let description = boot_description(id);
            let prefix = if index == state.selected { ">" } else { " " };
            uefi::println!("{prefix} Boot{id:04X}: {description}");
            log::info!(
                "AW_UEFI_MENU_ITEM page=BootPriorities index={} selected={} boot={:04X} label=\"{}\"",
                index,
                index == state.selected,
                id,
                description
            );
        }
        let back_index = state.boot_order.len();
        let prefix = if back_index == state.selected { ">" } else { " " };
        uefi::println!("{prefix} Back");
        log::info!(
            "AW_UEFI_MENU_ITEM page=BootPriorities index={} selected={} label=\"Back\"",
            back_index,
            back_index == state.selected
        );
    } else {
        for (index, item) in static_items(state.page).iter().enumerate() {
            let prefix = if index == state.selected { ">" } else { " " };
            uefi::println!("{prefix} {}", item.label);
            log::info!(
                "AW_UEFI_MENU_ITEM page={:?} index={} selected={} label=\"{}\"",
                state.page,
                index,
                index == state.selected,
                item.label
            );
        }
    }

    log_value(state, width, height);
}

fn log_value(state: &SetupState, width: usize, height: usize) {
    match (state.page, state.selected) {
        (Page::Main, 1) => {
            log::info!("AW_UEFI_SETUP_VALUE name=cpu_model value=\"{}\"", cpu_model());
        }
        (Page::Main, 2) => match runtime::get_time() {
            Ok(time) => log::info!("AW_UEFI_SETUP_VALUE name=firmware_time value={time:?}"),
            Err(error) => log::warn!(
                "AW_UEFI_SETUP_VALUE name=firmware_time unavailable status={:?}",
                error.status()
            ),
        },
        (Page::SystemInformation, 0) => {
            log::info!(
                "AW_UEFI_SETUP_VALUE name=firmware_vendor value=\"{}\"",
                system::firmware_vendor()
            );
        }
        (Page::SystemInformation, 1) => {
            log::info!(
                "AW_UEFI_SETUP_VALUE name=firmware_revision value={}",
                system::firmware_revision()
            );
        }
        (Page::SystemInformation, 2) => {
            log::info!(
                "AW_UEFI_SETUP_VALUE name=uefi_revision value={}",
                system::uefi_revision()
            );
        }
        (Page::SystemInformation, 3) => {
            log::info!(
                "AW_UEFI_SETUP_VALUE name=display value={}x{}",
                width,
                height
            );
        }
        (Page::SystemInformation, 4) => match read_u16(cstr16!("BootCurrent")) {
            Some(id) => log::info!(
                "AW_UEFI_SETUP_VALUE name=boot_current value=Boot{:04X}",
                id
            ),
            None => log::warn!("AW_UEFI_SETUP_VALUE name=boot_current unavailable=true"),
        },
        (Page::CpuConfiguration, 0) => {
            log::info!("AW_UEFI_SETUP_VALUE name=architecture value=x86_64");
        }
        (Page::CpuConfiguration, 1) => {
            log::info!(
                "AW_UEFI_SETUP_VALUE name=virtualization supported={}",
                virtualization_supported()
            );
        }
        (Page::SecureBoot, 0) => match read_flag(cstr16!("SecureBoot")) {
            Some(value) => log::info!("AW_UEFI_SETUP_VALUE name=secure_boot enabled={value}"),
            None => log::warn!("AW_UEFI_SETUP_VALUE name=secure_boot unavailable=true"),
        },
        (Page::SecureBoot, 1) => match read_flag(cstr16!("SetupMode")) {
            Some(value) => log::info!("AW_UEFI_SETUP_VALUE name=setup_mode enabled={value}"),
            None => log::warn!("AW_UEFI_SETUP_VALUE name=setup_mode unavailable=true"),
        },
        (Page::BootEntry, _) => {
            if let Some(id) = state.selected_boot {
                log::info!(
                    "AW_UEFI_SETUP_VALUE name=selected_boot id={:04X} label=\"{}\"",
                    id,
                    boot_description(id)
                );
            }
        }
        _ => {}
    }
}

fn say_text_value(text: &str, speaker: &mut Option<hda::Speaker>) {
    setup_speech::say(Clip::Value, speaker);
    setup_speech::spell(text, speaker);
}

fn speak_value(state: &SetupState, width: usize, height: usize, speaker: &mut Option<hda::Speaker>) {
    match (state.page, state.selected) {
        (Page::Main, 1) => say_text_value(&cpu_model(), speaker),
        (Page::Main, 2) => match runtime::get_time() {
            Ok(time) => say_text_value(&format!("{time:?}"), speaker),
            Err(_) => setup_speech::say(Clip::Unavailable, speaker),
        },
        (Page::SystemInformation, 0) => {
            say_text_value(&system::firmware_vendor().to_string(), speaker);
        }
        (Page::SystemInformation, 1) => {
            say_text_value(&system::firmware_revision().to_string(), speaker);
        }
        (Page::SystemInformation, 2) => {
            say_text_value(&system::uefi_revision().to_string(), speaker);
        }
        (Page::SystemInformation, 3) => {
            say_text_value(&format!("{width} x {height}"), speaker);
        }
        (Page::SystemInformation, 4) => match read_u16(cstr16!("BootCurrent")) {
            Some(id) => {
                let description = boot_description(id);
                say_text_value(&format!("Boot{id:04X} {description}"), speaker);
            }
            None => setup_speech::say(Clip::Unavailable, speaker),
        },
        (Page::CpuConfiguration, 0) => setup_speech::spell("x86 64", speaker),
        (Page::CpuConfiguration, 1) => {
            let clip = if virtualization_supported() {
                Clip::Supported
            } else {
                Clip::NotSupported
            };
            setup_speech::say(clip, speaker);
        }
        (Page::SecureBoot, 0) => match read_flag(cstr16!("SecureBoot")) {
            Some(true) => setup_speech::say(Clip::Enabled, speaker),
            Some(false) => setup_speech::say(Clip::Disabled, speaker),
            None => setup_speech::say(Clip::Unavailable, speaker),
        },
        (Page::SecureBoot, 1) => match read_flag(cstr16!("SetupMode")) {
            Some(true) => setup_speech::say(Clip::Enabled, speaker),
            Some(false) => setup_speech::say(Clip::Disabled, speaker),
            None => setup_speech::say(Clip::Unavailable, speaker),
        },
        (Page::BootEntry, _) => {
            if let Some(id) = state.selected_boot {
                setup_speech::say(Clip::Selected, speaker);
                setup_speech::say(Clip::BootOption, speaker);
                setup_speech::spell(&format!("{id:04X} {}", boot_description(id)), speaker);
            }
        }
        _ => {}
    }
}

fn announce_focus(
    state: &SetupState,
    width: usize,
    height: usize,
    speaker: &mut Option<hda::Speaker>,
) {
    if state.page == Page::BootPriorities {
        if state.selected < state.boot_order.len() {
            let id = state.boot_order[state.selected];
            let description = boot_description(id);
            log::info!(
                "AW_UEFI_SETUP_FOCUS page=BootPriorities index={} boot={:04X} label=\"{}\"",
                state.selected,
                id,
                description
            );
            setup_speech::say(Clip::BootOption, speaker);
            setup_speech::spell(&format!("{id:04X} {description}"), speaker);
        } else {
            log::info!(
                "AW_UEFI_SETUP_FOCUS page=BootPriorities index={} label=\"Back\"",
                state.selected
            );
            setup_speech::say(Clip::Back, speaker);
        }
        return;
    }

    let item = &static_items(state.page)[state.selected];
    log::info!(
        "AW_UEFI_SETUP_FOCUS page={:?} index={} label=\"{}\"",
        state.page,
        state.selected,
        item.label
    );
    setup_speech::say(item.clip, speaker);
    log_value(state, width, height);
    speak_value(state, width, height, speaker);
}

fn move_selection(state: &SetupState, delta: i8) -> usize {
    let count = item_count(state);
    if count == 0 {
        return 0;
    }
    if delta < 0 {
        if state.selected == 0 {
            count - 1
        } else {
            state.selected - 1
        }
    } else {
        (state.selected + 1) % count
    }
}

fn enter_page(
    state: &mut SetupState,
    page: Page,
    width: usize,
    height: usize,
    speaker: &mut Option<hda::Speaker>,
) {
    state.page = page;
    state.selected = 0;
    sound::cue(CUE_ENTER_HZ, Duration::from_millis(45));
    log::info!("AW_UEFI_SETUP_ACTION action=open page={:?}", page);
    render(state, width, height);
    announce_focus(state, width, height, speaker);
}

fn go_back(
    state: &mut SetupState,
    width: usize,
    height: usize,
    speaker: &mut Option<hda::Speaker>,
) {
    state.page = parent(state.page);
    state.selected = 0;
    if state.page != Page::BootEntry {
        state.selected_boot = None;
    }
    sound::cue(CUE_BACK_HZ, Duration::from_millis(45));
    log::info!("AW_UEFI_SETUP_ACTION action=back page={:?}", state.page);
    render(state, width, height);
    announce_focus(state, width, height, speaker);
}

fn activate(
    state: &mut SetupState,
    width: usize,
    height: usize,
    speaker: &mut Option<hda::Speaker>,
) -> bool {
    if state.page == Page::BootPriorities {
        if state.selected < state.boot_order.len() {
            state.selected_boot = Some(state.boot_order[state.selected]);
            enter_page(state, Page::BootEntry, width, height, speaker);
        } else {
            go_back(state, width, height, speaker);
        }
        return false;
    }

    let item = static_items(state.page)[state.selected];
    match item.action {
        Action::Enter(next) => enter_page(state, next, width, height, speaker),
        Action::Back => go_back(state, width, height, speaker),
        Action::ContinueBoot => {
            log::info!("AW_UEFI_SETUP_ACTION action=boot_normally reason=user");
            log::info!("AW_UEFI_SETUP_PROOF_OK");
            return true;
        }
        Action::Restart => {
            setup_speech::say(Clip::ActionSucceeded, speaker);
            log::info!("AW_UEFI_SETUP_ACTION action=restart confirmed=true");
            runtime::reset(ResetType::COLD, Status::SUCCESS, None);
        }
        Action::Shutdown => {
            setup_speech::say(Clip::ActionSucceeded, speaker);
            log::info!("AW_UEFI_SETUP_ACTION action=shutdown confirmed=true");
            runtime::reset(ResetType::SHUTDOWN, Status::SUCCESS, None);
        }
        Action::BootNow => {
            let Some(id) = state.selected_boot else {
                setup_speech::say(Clip::ActionFailed, speaker);
                log::error!("AW_UEFI_SETUP_ACTION action=boot_now status=no_selection");
                return false;
            };
            if set_boot_next(id) {
                setup_speech::say(Clip::ActionSucceeded, speaker);
                log::info!(
                    "AW_UEFI_SETUP_ACTION action=boot_now boot={:04X} status=success",
                    id
                );
                runtime::reset(ResetType::COLD, Status::SUCCESS, None);
            } else {
                setup_speech::say(Clip::ActionFailed, speaker);
                log::error!(
                    "AW_UEFI_SETUP_ACTION action=boot_now boot={:04X} status=failed",
                    id
                );
            }
        }
        Action::SetAsDefault => {
            let Some(id) = state.selected_boot else {
                setup_speech::say(Clip::ActionFailed, speaker);
                log::error!("AW_UEFI_SETUP_ACTION action=set_default status=no_selection");
                return false;
            };
            if set_boot_default(id, state) {
                setup_speech::say(Clip::ActionSucceeded, speaker);
                log::info!(
                    "AW_UEFI_SETUP_ACTION action=set_default boot={:04X} status=success",
                    id
                );
            } else {
                setup_speech::say(Clip::ActionFailed, speaker);
                log::error!(
                    "AW_UEFI_SETUP_ACTION action=set_default boot={:04X} status=failed",
                    id
                );
            }
        }
        Action::Noop => announce_focus(state, width, height, speaker),
    }
    false
}

/// Run the accessible Setup before ExitBootServices.
///
/// Unattended boots wait briefly and continue. Once any key is received, the
/// timeout is permanently disabled for this boot so nonvisual navigation is never
/// raced by automatic startup.
pub fn run(width: usize, height: usize, mut speaker: Option<hda::Speaker>) {
    log::info!("AW_UEFI_SETUP_BEGIN");
    setup_speech::log_mode();
    setup_speech::say(Clip::Setup, &mut speaker);
    setup_speech::say(Clip::InstructionsNavigation, &mut speaker);
    setup_speech::say(Clip::InstructionsSelect, &mut speaker);
    setup_speech::say(Clip::InstructionsTimeout, &mut speaker);

    let mut state = SetupState {
        page: Page::Root,
        selected: 0,
        selected_boot: None,
        boot_order: load_boot_order(),
    };
    let mut interacted = false;
    let mut waited = Duration::ZERO;

    log::info!(
        "AW_UEFI_SETUP_BOOT_OPTIONS count={}",
        state.boot_order.len()
    );
    render(&state, width, height);
    announce_focus(&state, width, height, &mut speaker);
    log::info!("AW_UEFI_SETUP_READY");

    loop {
        if let Some(key) = poll_key() {
            interacted = true;
            match key {
                Key::Special(ScanCode::DOWN) => {
                    state.selected = move_selection(&state, 1);
                    sound::cue(CUE_MOVE_HZ, Duration::from_millis(30));
                    render(&state, width, height);
                    announce_focus(&state, width, height, &mut speaker);
                }
                Key::Special(ScanCode::UP) => {
                    state.selected = move_selection(&state, -1);
                    sound::cue(CUE_MOVE_HZ, Duration::from_millis(30));
                    render(&state, width, height);
                    announce_focus(&state, width, height, &mut speaker);
                }
                Key::Special(ScanCode::ESCAPE) => {
                    if state.page == Page::Root {
                        log::info!(
                            "AW_UEFI_SETUP_ACTION action=boot_normally reason=escape_root"
                        );
                        log::info!("AW_UEFI_SETUP_PROOF_OK");
                        return;
                    }
                    go_back(&mut state, width, height, &mut speaker);
                }
                Key::Printable(character) => match char::from(character) {
                    '\r' => {
                        if activate(&mut state, width, height, &mut speaker) {
                            return;
                        }
                    }
                    '\t' => {
                        state.selected = move_selection(&state, 1);
                        sound::cue(CUE_MOVE_HZ, Duration::from_millis(30));
                        render(&state, width, height);
                        announce_focus(&state, width, height, &mut speaker);
                    }
                    _ => {}
                },
                Key::Special(_) => {}
            }
            continue;
        }

        if !interacted {
            if waited >= IDLE_WINDOW {
                log::info!("AW_UEFI_SETUP_ACTION action=boot_normally reason=timeout");
                log::info!("AW_UEFI_SETUP_PROOF_OK");
                return;
            }
            waited += POLL_INTERVAL;
        }
        boot::stall(POLL_INTERVAL);
    }
}
