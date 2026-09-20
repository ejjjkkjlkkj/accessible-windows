fn enter_page(
    page: &mut Page,
    selected: &mut usize,
    next: Page,
    options: &[BootOption],
    width: usize,
    height: usize,
    speaker: &mut Option<hda::Speaker>,
) {
    *page = next;
    *selected = 0;
    sound::cue(CUE_ENTER_HZ, Duration::from_millis(45));
    log::info!("AW_UEFI_SETUP_ACTION action=open page={:?}", next);
    render(*page, *selected, options, width, height);
    speak_page(*page, options, speaker);
    announce_focus(*page, *selected, options, width, height, speaker);
}

fn go_back(
    page: &mut Page,
    selected: &mut usize,
    options: &[BootOption],
    width: usize,
    height: usize,
    speaker: &mut Option<hda::Speaker>,
) {
    *page = parent(*page);
    *selected = 0;
    sound::cue(CUE_BACK_HZ, Duration::from_millis(45));
    log::info!("AW_UEFI_SETUP_ACTION action=back page={:?}", *page);
    render(*page, *selected, options, width, height);
    speak_page(*page, options, speaker);
    announce_focus(*page, *selected, options, width, height, speaker);
}

fn activate(
    page: &mut Page,
    selected: &mut usize,
    options: &mut Vec<BootOption>,
    width: usize,
    height: usize,
    speaker: &mut Option<hda::Speaker>,
) -> bool {
    match (*page, *selected) {
        (Page::Root, 0) => {
            enter_page(page, selected, Page::Main, options, width, height, speaker)
        }
        (Page::Root, 1) => {
            enter_page(page, selected, Page::Advanced, options, width, height, speaker)
        }
        (Page::Root, 2) => {
            enter_page(page, selected, Page::Boot, options, width, height, speaker)
        }
        (Page::Root, 3) => {
            enter_page(page, selected, Page::Security, options, width, height, speaker)
        }
        (Page::Root, 4) => {
            enter_page(page, selected, Page::SaveExit, options, width, height, speaker)
        }
        (Page::Main, 0) => enter_page(
            page,
            selected,
            Page::SystemInformation,
            options,
            width,
            height,
            speaker,
        ),
        (Page::Main, 1) => go_back(page, selected, options, width, height, speaker),
        (Page::SystemInformation, 0..=6) => {
            announce_focus(*page, *selected, options, width, height, speaker);
        }
        (Page::SystemInformation, 7) => {
            go_back(page, selected, options, width, height, speaker)
        }
        (Page::Advanced, 0) => enter_page(
            page,
            selected,
            Page::CpuConfiguration,
            options,
            width,
            height,
            speaker,
        ),
        (Page::Advanced, 1) => go_back(page, selected, options, width, height, speaker),
        (Page::CpuConfiguration, 0..=1) => {
            announce_focus(*page, *selected, options, width, height, speaker);
        }
        (Page::CpuConfiguration, 2) => {
            go_back(page, selected, options, width, height, speaker)
        }
        (Page::Boot, 0) => enter_page(
            page,
            selected,
            Page::BootPriorities,
            options,
            width,
            height,
            speaker,
        ),
        (Page::Boot, 1) | (Page::SaveExit, 0) => {
            log::info!("AW_UEFI_SETUP_ACTION action=boot_normally reason=user");
            setup_speech::say(Clip::BootNormally, speaker);
            log::info!("AW_UEFI_SETUP_PROOF_OK");
            return true;
        }
        (Page::Boot, 2) => go_back(page, selected, options, width, height, speaker),
        (Page::BootPriorities, index) if index < options.len() => enter_page(
            page,
            selected,
            Page::BootOption(index),
            options,
            width,
            height,
            speaker,
        ),
        (Page::BootPriorities, index) if index + 1 == item_count(*page, options) => {
            go_back(page, selected, options, width, height, speaker)
        }
        (Page::BootPriorities, _) => {
            setup_speech::say(Clip::Unavailable, speaker);
        }
        (Page::BootOption(index), 0) => {
            let number = options.get(index).map(|option| option.number);
            let ok = number.map(set_boot_next).unwrap_or(false);
            action_result("boot_next", ok, speaker);
            if ok {
                runtime::reset(ResetType::COLD, Status::SUCCESS, None);
            }
        }
        (Page::BootOption(index), 1) => {
            let number = options.get(index).map(|option| option.number);
            let ok = number.map(set_boot_default).unwrap_or(false);
            action_result("set_boot_default", ok, speaker);
            if ok && index < options.len() {
                let chosen = options.remove(index);
                options.insert(0, chosen);
                *page = Page::BootPriorities;
                *selected = 0;
                render(*page, *selected, options, width, height);
                speak_page(*page, options, speaker);
                announce_focus(*page, *selected, options, width, height, speaker);
            }
        }
        (Page::BootOption(_), 2) => go_back(page, selected, options, width, height, speaker),
        (Page::Security, 0) => enter_page(
            page,
            selected,
            Page::SecureBoot,
            options,
            width,
            height,
            speaker,
        ),
        (Page::Security, 1) => go_back(page, selected, options, width, height, speaker),
        (Page::SecureBoot, 0..=1) => {
            announce_focus(*page, *selected, options, width, height, speaker);
        }
        (Page::SecureBoot, 2) => go_back(page, selected, options, width, height, speaker),
        (Page::SaveExit, 1) => enter_page(
            page,
            selected,
            Page::ConfirmRestart,
            options,
            width,
            height,
            speaker,
        ),
        (Page::SaveExit, 2) => enter_page(
            page,
            selected,
            Page::ConfirmShutdown,
            options,
            width,
            height,
            speaker,
        ),
        (Page::SaveExit, 3) => go_back(page, selected, options, width, height, speaker),
        (Page::ConfirmRestart, 0) => {
            action_result("restart", true, speaker);
            runtime::reset(ResetType::COLD, Status::SUCCESS, None);
        }
        (Page::ConfirmRestart, 1) => go_back(page, selected, options, width, height, speaker),
        (Page::ConfirmShutdown, 0) => {
            action_result("shutdown", true, speaker);
            runtime::reset(ResetType::SHUTDOWN, Status::SUCCESS, None);
        }
        (Page::ConfirmShutdown, 1) => go_back(page, selected, options, width, height, speaker),
        _ => {}
    }
    false
}
