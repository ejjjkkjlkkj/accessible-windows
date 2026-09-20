fn static_item(page: Page, index: usize) -> Option<(&'static str, Clip)> {
    match (page, index) {
        (Page::Root, 0) => Some(("Main", Clip::Main)),
        (Page::Root, 1) => Some(("Advanced", Clip::Advanced)),
        (Page::Root, 2) => Some(("Boot", Clip::Boot)),
        (Page::Root, 3) => Some(("Security", Clip::Security)),
        (Page::Root, 4) => Some(("Save and Exit", Clip::SaveExit)),
        (Page::Main, 0) => Some(("System information", Clip::SystemInformation)),
        (Page::Main, 1) => Some(("Back", Clip::Back)),
        (Page::SystemInformation, 0) => Some(("CPU model", Clip::CpuModel)),
        (Page::SystemInformation, 1) => Some(("Firmware vendor", Clip::FirmwareVendor)),
        (Page::SystemInformation, 2) => Some(("Firmware revision", Clip::FirmwareRevision)),
        (Page::SystemInformation, 3) => Some(("UEFI revision", Clip::UefiRevision)),
        (Page::SystemInformation, 4) => Some(("Firmware time", Clip::FirmwareTime)),
        (Page::SystemInformation, 5) => Some(("Display resolution", Clip::DisplayResolution)),
        (Page::SystemInformation, 6) => Some(("Current boot option", Clip::BootCurrent)),
        (Page::SystemInformation, 7) => Some(("Back", Clip::Back)),
        (Page::Advanced, 0) => Some(("CPU configuration", Clip::CpuConfiguration)),
        (Page::Advanced, 1) => Some(("Back", Clip::Back)),
        (Page::CpuConfiguration, 0) => Some(("Architecture", Clip::Architecture)),
        (Page::CpuConfiguration, 1) => {
            Some(("Virtualization capability", Clip::VirtualizationCapability))
        }
        (Page::CpuConfiguration, 2) => Some(("Back", Clip::Back)),
        (Page::Boot, 0) => Some(("Boot option priorities", Clip::BootOptionPriorities)),
        (Page::Boot, 1) => Some(("Boot normally", Clip::BootNormally)),
        (Page::Boot, 2) => Some(("Back", Clip::Back)),
        (Page::BootOption(_), 0) => Some(("Boot now", Clip::BootNow)),
        (Page::BootOption(_), 1) => Some(("Set as default", Clip::SetAsDefault)),
        (Page::BootOption(_), 2) => Some(("Back", Clip::Back)),
        (Page::Security, 0) => Some(("Secure Boot", Clip::SecureBoot)),
        (Page::Security, 1) => Some(("Back", Clip::Back)),
        (Page::SecureBoot, 0) => Some(("Secure Boot status", Clip::SecureBootStatus)),
        (Page::SecureBoot, 1) => Some(("Setup mode", Clip::SetupMode)),
        (Page::SecureBoot, 2) => Some(("Back", Clip::Back)),
        (Page::SaveExit, 0) => Some(("Boot normally", Clip::BootNormally)),
        (Page::SaveExit, 1) => Some(("Restart", Clip::ResetSystem)),
        (Page::SaveExit, 2) => Some(("Shut down", Clip::ShutDown)),
        (Page::SaveExit, 3) => Some(("Back", Clip::Back)),
        (Page::ConfirmRestart, 0) => Some(("Confirm restart", Clip::ConfirmRestart)),
        (Page::ConfirmRestart, 1) => Some(("Cancel", Clip::Cancel)),
        (Page::ConfirmShutdown, 0) => Some(("Confirm shut down", Clip::ConfirmShutdown)),
        (Page::ConfirmShutdown, 1) => Some(("Cancel", Clip::Cancel)),
        _ => None,
    }
}

fn boot_priorities_base(options: &[BootOption]) -> usize {
    options.len().max(1)
}

fn item_count(page: Page, options: &[BootOption]) -> usize {
    match page {
        Page::Root => 5,
        Page::Main => 2,
        Page::SystemInformation => 8,
        Page::Advanced => 2,
        Page::CpuConfiguration => 3,
        Page::Boot => 3,
        Page::BootPriorities => boot_priorities_base(options) + 1,
        Page::BootOption(_) => 3,
        Page::Security => 2,
        Page::SecureBoot => 3,
        Page::SaveExit => 4,
        Page::ConfirmRestart | Page::ConfirmShutdown => 2,
    }
}

fn item_text<'a>(page: Page, index: usize, options: &'a [BootOption]) -> &'a str {
    if page == Page::BootPriorities {
        if let Some(option) = options.get(index) {
            return option.description.as_str();
        }
        if index + 1 == item_count(page, options) {
            return "Back";
        }
        return "Unavailable";
    }
    static_item(page, index)
        .map(|(label, _)| label)
        .unwrap_or("Unavailable")
}

fn current_boot_value(options: &[BootOption]) -> String {
    read_u16(cstr16!("BootCurrent"))
        .map(|number| {
            options
                .iter()
                .find(|option| option.number == number)
                .map(|option| format!("Boot {number:04X} {}", option.description))
                .unwrap_or_else(|| format!("Boot {number:04X}"))
        })
        .unwrap_or_else(|| "Unavailable".to_string())
}

fn value_for(
    page: Page,
    index: usize,
    options: &[BootOption],
    width: usize,
    height: usize,
) -> Option<String> {
    match (page, index) {
        (Page::SystemInformation, 0) => Some(cpu_brand()),
        (Page::SystemInformation, 1) => Some(system::firmware_vendor().to_string()),
        (Page::SystemInformation, 2) => Some(system::firmware_revision().to_string()),
        (Page::SystemInformation, 3) => Some(system::uefi_revision().to_string()),
        (Page::SystemInformation, 4) => Some(
            runtime::get_time()
                .map(|time| {
                    format!(
                        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                        time.year(),
                        time.month(),
                        time.day(),
                        time.hour(),
                        time.minute(),
                        time.second()
                    )
                })
                .unwrap_or_else(|_| "Unavailable".to_string()),
        ),
        (Page::SystemInformation, 5) => Some(format!("{width} by {height}")),
        (Page::SystemInformation, 6) => Some(current_boot_value(options)),
        (Page::CpuConfiguration, 0) => Some("x86_64".to_string()),
        (Page::CpuConfiguration, 1) => Some(
            if virtualization_capability() {
                "Supported"
            } else {
                "Not supported"
            }
            .to_string(),
        ),
        (Page::SecureBoot, 0) => Some(
            read_flag(cstr16!("SecureBoot"))
                .map(|enabled| if enabled { "Enabled" } else { "Disabled" })
                .unwrap_or("Unavailable")
                .to_string(),
        ),
        (Page::SecureBoot, 1) => Some(
            read_flag(cstr16!("SetupMode"))
                .map(|enabled| if enabled { "Enabled" } else { "Disabled" })
                .unwrap_or("Unavailable")
                .to_string(),
        ),
        _ => None,
    }
}

fn render(page: Page, selected: usize, options: &[BootOption], width: usize, height: usize) {
    let _ = system::with_stdout(|stdout| stdout.clear());
    let (title, _) = page_title(page);
    uefi::println!("Accessible Windows - {title}");
    uefi::println!();
    uefi::println!("Up/Down or Tab: move. Enter: open/activate. Escape: back.");
    uefi::println!();

    for index in 0..item_count(page, options) {
        let prefix = if index == selected { '>' } else { ' ' };
        let label = item_text(page, index, options);
        if let Some(value) = value_for(page, index, options, width, height) {
            uefi::println!("{prefix} {label}: {value}");
        } else {
            uefi::println!("{prefix} {label}");
        }
    }
}

fn speak_page(page: Page, options: &[BootOption], speaker: &mut Option<hda::Speaker>) {
    let (title, clip) = page_title(page);
    log::info!("AW_UEFI_SETUP_PAGE page={:?} title=\"{}\"", page, title);
    setup_speech::say(clip, speaker);
    if let Page::BootOption(index) = page {
        if let Some(option) = options.get(index) {
            setup_speech::spell(&option.description, speaker);
        }
    }
}

fn speak_known_value(value: &str, speaker: &mut Option<hda::Speaker>) {
    match value {
        "Enabled" => setup_speech::say(Clip::Enabled, speaker),
        "Disabled" => setup_speech::say(Clip::Disabled, speaker),
        "Unavailable" => setup_speech::say(Clip::Unavailable, speaker),
        "Supported" => setup_speech::say(Clip::Supported, speaker),
        "Not supported" => setup_speech::say(Clip::NotSupported, speaker),
        _ => {
            setup_speech::say(Clip::Value, speaker);
            setup_speech::spell(value, speaker);
        }
    }
}

fn announce_focus(
    page: Page,
    selected: usize,
    options: &[BootOption],
    width: usize,
    height: usize,
    speaker: &mut Option<hda::Speaker>,
) {
    let label = item_text(page, selected, options);
    log::info!(
        "AW_UEFI_MENU_ITEM page={:?} index={} label=\"{}\"",
        page,
        selected,
        label
    );
    setup_speech::say(Clip::Selected, speaker);

    if page == Page::BootPriorities {
        if let Some(option) = options.get(selected) {
            setup_speech::say(Clip::BootOption, speaker);
            setup_speech::spell(&option.description, speaker);
        } else if selected + 1 == item_count(page, options) {
            setup_speech::say(Clip::Back, speaker);
        } else {
            setup_speech::say(Clip::Unavailable, speaker);
        }
    } else if let Some((_, clip)) = static_item(page, selected) {
        setup_speech::say(clip, speaker);
    }

    if let Some(value) = value_for(page, selected, options, width, height) {
        log::info!(
            "AW_UEFI_SETUP_VALUE page={:?} index={} value=\"{}\"",
            page,
            selected,
            value
        );
        speak_known_value(&value, speaker);
    }
}

fn move_selection(page: Page, selected: usize, delta: i8, options: &[BootOption]) -> usize {
    let count = item_count(page, options);
    if delta < 0 {
        if selected == 0 {
            count - 1
        } else {
            selected - 1
        }
    } else {
        (selected + 1) % count
    }
}

fn action_result(name: &str, ok: bool, speaker: &mut Option<hda::Speaker>) {
    log::info!(
        "AW_UEFI_SETUP_ACTION action={name} result={}",
        if ok { "ok" } else { "fail" }
    );
    if ok {
        setup_speech::say(Clip::ActionSucceeded, speaker);
    } else {
        setup_speech::say(Clip::ActionFailed, speaker);
    }
}
