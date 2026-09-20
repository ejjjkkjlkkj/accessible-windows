//! Embedded speech assets for the accessible firmware setup.
//!
//! Fixed labels are spoken as complete phrases. Dynamic values (boot entry
//! descriptions, CPU model, time, resolution) fall back to an alphanumeric
//! spelling bank so every value remains nonvisual without requiring a runtime
//! speech synthesizer in firmware.

use core::time::Duration;

use uefi::boot;

use crate::hda;

#[derive(Clone, Copy)]
pub enum Clip {
    Setup,
    Main,
    Advanced,
    Boot,
    Security,
    SaveExit,
    CpuModel,
    FirmwareTime,
    DisplayInformation,
    BootCurrent,
    CpuConfiguration,
    VirtualizationCapability,
    BootOptionPriorities,
    BootNow,
    SetAsDefault,
    SecureBoot,
    SecureBootStatus,
    SetupMode,
    BootNormally,
    ResetSystem,
    ShutDown,
    Back,
    Enabled,
    Disabled,
    Unavailable,
    Selected,
    BootOption,
    ActionSucceeded,
    ActionFailed,
    Value,
    InstructionsNavigation,
    InstructionsSelect,
    InstructionsTimeout,
}

macro_rules! asset {
    ($name:literal) => {
        include_bytes!(concat!(env!("OUT_DIR"), "/aw-speech/", $name, ".pcm"))
    };
}

pub fn real_assets() -> bool {
    option_env!("AW_SETUP_SPEECH_REAL") == Some("1")
}

pub fn log_mode() {
    if real_assets() {
        log::info!("AW_UEFI_SETUP_SPEECH_MODE mode=real");
    } else {
        log::warn!("AW_UEFI_SETUP_SPEECH_MODE mode=fallback");
    }
}

pub fn clip(value: Clip) -> &'static [u8] {
    match value {
        Clip::Setup => asset!("setup_setup"),
        Clip::Main => asset!("setup_main"),
        Clip::Advanced => asset!("setup_advanced"),
        Clip::Boot => asset!("setup_boot"),
        Clip::Security => asset!("setup_security"),
        Clip::SaveExit => asset!("setup_save_exit"),
        Clip::CpuModel => asset!("setup_cpu_model"),
        Clip::FirmwareTime => asset!("setup_firmware_time"),
        Clip::DisplayInformation => asset!("setup_display_information"),
        Clip::BootCurrent => asset!("setup_boot_current"),
        Clip::CpuConfiguration => asset!("setup_cpu_configuration"),
        Clip::VirtualizationCapability => asset!("setup_virtualization_capability"),
        Clip::BootOptionPriorities => asset!("setup_boot_option_priorities"),
        Clip::BootNow => asset!("setup_boot_now"),
        Clip::SetAsDefault => asset!("setup_set_as_default"),
        Clip::SecureBoot => asset!("setup_secure_boot"),
        Clip::SecureBootStatus => asset!("setup_secure_boot_status"),
        Clip::SetupMode => asset!("setup_setup_mode"),
        Clip::BootNormally => asset!("setup_boot_normally"),
        Clip::ResetSystem => asset!("setup_reset_system"),
        Clip::ShutDown => asset!("setup_shut_down"),
        Clip::Back => asset!("setup_back"),
        Clip::Enabled => asset!("setup_enabled"),
        Clip::Disabled => asset!("setup_disabled"),
        Clip::Unavailable => asset!("setup_unavailable"),
        Clip::Selected => asset!("setup_selected"),
        Clip::BootOption => asset!("setup_boot_option"),
        Clip::ActionSucceeded => asset!("setup_action_succeeded"),
        Clip::ActionFailed => asset!("setup_action_failed"),
        Clip::Value => asset!("setup_value"),
        Clip::InstructionsNavigation => asset!("setup_instructions_navigation"),
        Clip::InstructionsSelect => asset!("setup_instructions_select"),
        Clip::InstructionsTimeout => asset!("setup_instructions_timeout"),
    }
}

fn spelling_clip(character: char) -> Option<&'static [u8]> {
    match character.to_ascii_lowercase() {
        'a' => Some(asset!("spell_a")),
        'b' => Some(asset!("spell_b")),
        'c' => Some(asset!("spell_c")),
        'd' => Some(asset!("spell_d")),
        'e' => Some(asset!("spell_e")),
        'f' => Some(asset!("spell_f")),
        'g' => Some(asset!("spell_g")),
        'h' => Some(asset!("spell_h")),
        'i' => Some(asset!("spell_i")),
        'j' => Some(asset!("spell_j")),
        'k' => Some(asset!("spell_k")),
        'l' => Some(asset!("spell_l")),
        'm' => Some(asset!("spell_m")),
        'n' => Some(asset!("spell_n")),
        'o' => Some(asset!("spell_o")),
        'p' => Some(asset!("spell_p")),
        'q' => Some(asset!("spell_q")),
        'r' => Some(asset!("spell_r")),
        's' => Some(asset!("spell_s")),
        't' => Some(asset!("spell_t")),
        'u' => Some(asset!("spell_u")),
        'v' => Some(asset!("spell_v")),
        'w' => Some(asset!("spell_w")),
        'x' => Some(asset!("spell_x")),
        'y' => Some(asset!("spell_y")),
        'z' => Some(asset!("spell_z")),
        '0' => Some(asset!("spell_0")),
        '1' => Some(asset!("spell_1")),
        '2' => Some(asset!("spell_2")),
        '3' => Some(asset!("spell_3")),
        '4' => Some(asset!("spell_4")),
        '5' => Some(asset!("spell_5")),
        '6' => Some(asset!("spell_6")),
        '7' => Some(asset!("spell_7")),
        '8' => Some(asset!("spell_8")),
        '9' => Some(asset!("spell_9")),
        '-' => Some(asset!("spell_dash")),
        '.' => Some(asset!("spell_dot")),
        ':' => Some(asset!("spell_colon")),
        '/' | '\\' => Some(asset!("spell_slash")),
        '_' => Some(asset!("spell_underscore")),
        _ => None,
    }
}

pub fn say(value: Clip, speaker: &mut Option<hda::Speaker>) {
    if !real_assets() {
        return;
    }
    if let Some(speaker) = speaker.as_mut() {
        let data = clip(value);
        if speaker.speak(data) {
            log::info!("AW_UEFI_HDA_SPEAK bytes={}", data.len());
        }
    }
}

/// Speak arbitrary firmware text by spelling supported characters. This is a
/// deliberately complete fallback for values that cannot be pre-recorded at
/// build time. Spaces and unsupported punctuation become short pauses.
pub fn spell(text: &str, speaker: &mut Option<hda::Speaker>) {
    if !real_assets() {
        return;
    }
    let Some(speaker) = speaker.as_mut() else {
        return;
    };

    for character in text.chars() {
        if character.is_ascii_whitespace() {
            boot::stall(Duration::from_millis(90));
            continue;
        }
        if let Some(data) = spelling_clip(character)
            && speaker.speak(data)
        {
            log::info!("AW_UEFI_HDA_SPELL char={:?} bytes={}", character, data.len());
        } else {
            boot::stall(Duration::from_millis(60));
        }
    }
}
