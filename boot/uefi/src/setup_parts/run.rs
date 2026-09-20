fn poll_key() -> Option<Key> {
    system::with_stdin(|stdin| stdin.read_key().unwrap_or(None))
}

/// Run the accessible Setup before ExitBootServices.
///
/// With no input, the root menu remains active for a short deterministic window
/// and then boots normally. Once any key is received, the timeout is disabled for
/// this boot so a person navigating nonvisually is never raced by automatic boot.
pub fn run(width: usize, height: usize, mut speaker: Option<hda::Speaker>) {
    log::info!("AW_UEFI_SETUP_BEGIN");
    setup_speech::log_mode();

    let mut options = boot_options();
    log::info!("AW_UEFI_SETUP_BOOT_OPTIONS count={}", options.len());

    let mut page = Page::Root;
    let mut selected = 0_usize;
    let mut interacted = false;
    let mut waited = Duration::ZERO;

    render(page, selected, &options, width, height);
    speak_page(page, &options, &mut speaker);
    log::info!("AW_UEFI_SETUP_HELP");
    setup_speech::say(Clip::InstructionsNavigation, &mut speaker);
    setup_speech::say(Clip::InstructionsSelect, &mut speaker);
    setup_speech::say(Clip::InstructionsTimeout, &mut speaker);
    announce_focus(page, selected, &options, width, height, &mut speaker);
    log::info!("AW_UEFI_SETUP_READY");

    loop {
        if let Some(key) = poll_key() {
            interacted = true;
            match key {
                Key::Special(ScanCode::DOWN) => {
                    selected = move_selection(page, selected, 1, &options);
                    sound::cue(CUE_MOVE_HZ, Duration::from_millis(30));
                    render(page, selected, &options, width, height);
                    announce_focus(page, selected, &options, width, height, &mut speaker);
                }
                Key::Special(ScanCode::UP) => {
                    selected = move_selection(page, selected, -1, &options);
                    sound::cue(CUE_MOVE_HZ, Duration::from_millis(30));
                    render(page, selected, &options, width, height);
                    announce_focus(page, selected, &options, width, height, &mut speaker);
                }
                Key::Special(ScanCode::ESCAPE) => {
                    if page == Page::Root {
                        log::info!(
                            "AW_UEFI_SETUP_ACTION action=boot_normally reason=escape_root"
                        );
                        setup_speech::say(Clip::BootNormally, &mut speaker);
                        log::info!("AW_UEFI_SETUP_PROOF_OK");
                        return;
                    }
                    go_back(
                        &mut page,
                        &mut selected,
                        &options,
                        width,
                        height,
                        &mut speaker,
                    );
                }
                Key::Printable(character) => match char::from(character) {
                    '\r' => {
                        if activate(
                            &mut page,
                            &mut selected,
                            &mut options,
                            width,
                            height,
                            &mut speaker,
                        ) {
                            return;
                        }
                    }
                    '\t' => {
                        selected = move_selection(page, selected, 1, &options);
                        sound::cue(CUE_MOVE_HZ, Duration::from_millis(30));
                        render(page, selected, &options, width, height);
                        announce_focus(page, selected, &options, width, height, &mut speaker);
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
                setup_speech::say(Clip::BootNormally, &mut speaker);
                log::info!("AW_UEFI_SETUP_PROOF_OK");
                return;
            }
            waited += POLL_INTERVAL;
        }
        boot::stall(POLL_INTERVAL);
    }
}
