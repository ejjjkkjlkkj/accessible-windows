//! Firmware-stage screen reader: the first moment Accessible Windows speaks, and
//! the first surface a user can operate - before the kernel is even loaded.
//!
//! The kernel's screen-reader proof voices the installer's welcome dialog, but
//! that runs after ExitBootServices, once the operating system is up. This runs
//! earlier still, inside the UEFI boot application, so the accessibility contract
//! holds from the first stage the project controls. It does three things a real
//! screen reader must do, this early:
//!
//! 1. **Speaks on a surface a person can actually perceive.** The utterances are
//!    written to the UEFI text console (`ConOut`), which is the physical screen on
//!    real hardware, not only to the QEMU debug port. Each is also emitted on the
//!    debug console as an `AW_UEFI_SR_*` marker, the machine-checkable nonvisual
//!    delivery evidence the boot proofs assert.
//! 2. **Describes the real machine.** The display line carries the resolution the
//!    firmware actually reported through GOP, not a placeholder, so what is spoken
//!    matches what is there.
//! 3. **Is operable by keyboard, with no pointer.** After reading the boot screen
//!    top to bottom, it waits: the up and down arrows (or Tab) re-read the screen a
//!    line at a time, and Enter continues to the operating system. If nobody is
//!    there - an unattended or automated boot - it continues on its own after a
//!    short window, so the machine never hangs waiting for a key that will not come.
//!
//! Every utterance comes from the one allocation-free announcement engine the
//! kernel and installer use ([`aw_screen_reader`]) over the same validated
//! semantics ([`aw_accessibility`]): the wording cannot drift between boot,
//! installer and desktop, because there is one engine, unit-tested once and proven
//! here to run unchanged this early.

extern crate alloc;

use alloc::string::String;

use core::time::Duration;

use aw_accessibility::{validate_node, NodeId, Rect, Role, SemanticNode, State};
use aw_screen_reader::{announce_focus, FocusContext};
use uefi::boot;
use uefi::proto::console::text::{Key, ScanCode};
use uefi::system;

use crate::hda;
use crate::sound;

/// Pitch of the audible cue when the reading cursor moves during review.
const CUE_MOVE_HZ: u32 = 740;
/// Pitch of the audible cue confirming the boot is continuing.
const CUE_CONTINUE_HZ: u32 = 523;
/// Pitch of the audible cue when the boot screen is waiting for the user.
const CUE_READY_HZ: u32 = 880;

/// How long an unattended boot waits for a key before it continues on its own.
/// The QEMU proof harness presses no key, so it always waits this out and then
/// boots - which is why the automated boot proof must never depend on a keystroke.
const REVIEW_WINDOW: Duration = Duration::from_secs(5);
/// How often the review window polls for a keystroke.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Build one node of the firmware boot screen. Every node hangs off an implicit
/// root (id 0), the way the kernel proof frames its dialog.
fn node<'a>(id: u64, role: Role, name: &'a str) -> SemanticNode<'a> {
    SemanticNode {
        id: NodeId(id),
        parent: Some(NodeId(0)),
        role,
        name,
        description: "",
        value: "",
        state: State::from_bits(0),
        // The firmware screen is text a user hears, not a laid-out control
        // surface, but a plausible non-zero rectangle keeps every node valid.
        bounds: Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 32,
        },
    }
}

/// Compose the utterance for `node`, or `None` if it violates an accessibility
/// invariant (which for these compile-time-constant nodes means a code bug). The
/// utterance is written into the caller's buffer so nothing is allocated.
fn utterance<'b>(node: &SemanticNode<'_>, buffer: &'b mut [u8]) -> Option<&'b str> {
    if validate_node(node).is_err() {
        log::error!("AW_UEFI_SR_FAIL reason=invalid_node");
        return None;
    }
    let text = announce_focus(node, FocusContext::NONE, buffer);
    if text.is_empty() {
        log::error!("AW_UEFI_SR_FAIL reason=empty_utterance");
        return None;
    }
    Some(text)
}

/// Print one spoken line on the visible console: what a speech engine would say,
/// clean and without the logger's source-location noise.
fn show(text: &str) {
    uefi::println!("  {text}");
}

/// Play a line's pre-recorded speech clip through the HDA codec, when audio is
/// available and the line has one (the dynamic display line does not). This is
/// what a blind user actually hears - the words, on the machine's real speakers.
fn play_clip(clip: Option<&'static [u8]>, speaker: &mut Option<hda::Speaker>) {
    if let (Some(sp), Some(data)) = (speaker.as_mut(), clip)
        && sp.speak(data)
    {
        log::info!("AW_UEFI_HDA_SPEAK bytes={}", data.len());
    }
}

/// Speak one node for the first time: show it on the console, emit its
/// `AW_UEFI_SR_SPEAK` marker, and say it aloud through HDA. Returns false on an
/// invariant violation.
fn speak(
    node: &SemanticNode<'_>,
    clip: Option<&'static [u8]>,
    speaker: &mut Option<hda::Speaker>,
) -> bool {
    let mut buffer = [0u8; 128];
    let Some(text) = utterance(node, &mut buffer) else {
        return false;
    };
    show(text);
    log::info!("AW_UEFI_SR_SPEAK \"{text}\"");
    play_clip(clip, speaker);
    true
}

/// Re-read one node during keyboard review: shown again, marked `AW_UEFI_SR_REVIEW`
/// (distinct from the first reading), and said aloud again through HDA.
fn review(
    node: &SemanticNode<'_>,
    clip: Option<&'static [u8]>,
    speaker: &mut Option<hda::Speaker>,
) {
    let mut buffer = [0u8; 128];
    if let Some(text) = utterance(node, &mut buffer) {
        show(text);
        log::info!("AW_UEFI_SR_REVIEW \"{text}\"");
        play_clip(clip, speaker);
    }
}

/// Poll the UEFI console input once, without blocking. Any read error is treated
/// as "no key", so a flaky console cannot wedge the boot.
fn poll_key() -> Option<Key> {
    system::with_stdin(|stdin| stdin.read_key().unwrap_or(None))
}

/// What a reviewed keystroke asks for.
enum Command {
    /// Move the reading cursor forward (down arrow / Tab).
    Next,
    /// Move the reading cursor backward (up arrow).
    Previous,
    /// Leave the boot screen and continue to the operating system.
    Continue(&'static str),
    /// A key with no binding here; ignored.
    Ignore,
}

/// Map a keystroke to a boot-screen command.
fn classify(key: Key) -> Command {
    match key {
        Key::Special(ScanCode::DOWN) => Command::Next,
        Key::Special(ScanCode::UP) => Command::Previous,
        Key::Special(ScanCode::ESCAPE) => Command::Continue("escape"),
        Key::Printable(character) => match char::from(character) {
            '\r' => Command::Continue("enter"),
            '\t' => Command::Next,
            _ => Command::Ignore,
        },
        Key::Special(_) => Command::Ignore,
    }
}

/// Let the user review the boot screen by keyboard and choose when to continue.
///
/// `lines` is the boot screen already read aloud once; the cursor starts on its
/// last line. Up/Down (or Tab) re-read a line, Enter or Escape continues. Until
/// the first keystroke a countdown runs, and if it expires the boot continues on
/// its own; once the user has pressed anything, the countdown stops and the boot
/// waits for them - a person reading takes as long as they take.
fn review_and_continue(
    lines: &[SemanticNode<'_>],
    clips: &[Option<&'static [u8]>],
    speaker: &mut Option<hda::Speaker>,
) {
    log::info!("AW_UEFI_SR_READY");
    uefi::println!();
    uefi::println!("  Up/Down: review a line.  Enter: continue.  Continuing in 5 seconds.");
    // An audible "waiting for you" cue, so a blind user knows input is expected.
    sound::cue(CUE_READY_HZ, Duration::from_millis(90));

    if lines.is_empty() {
        log::info!("AW_UEFI_SR_CONTINUE reason=empty");
        return;
    }

    let last = lines.len() - 1;
    let mut cursor = last;
    let mut interacted = false;
    let mut waited = Duration::ZERO;

    loop {
        if let Some(key) = poll_key() {
            interacted = true;
            match classify(key) {
                Command::Next => {
                    cursor = (cursor + 1).min(last);
                    sound::cue(CUE_MOVE_HZ, Duration::from_millis(35));
                    review(&lines[cursor], clips[cursor], speaker);
                }
                Command::Previous => {
                    cursor = cursor.saturating_sub(1);
                    sound::cue(CUE_MOVE_HZ, Duration::from_millis(35));
                    review(&lines[cursor], clips[cursor], speaker);
                }
                Command::Continue(reason) => {
                    sound::cue(CUE_CONTINUE_HZ, Duration::from_millis(150));
                    log::info!("AW_UEFI_SR_CONTINUE reason={reason}");
                    return;
                }
                Command::Ignore => {}
            }
            continue;
        }

        // No key waiting. An unattended boot counts down and then continues; once
        // someone has interacted, the countdown is abandoned and we simply wait.
        if !interacted {
            if waited >= REVIEW_WINDOW {
                log::info!("AW_UEFI_SR_CONTINUE reason=timeout");
                return;
            }
            waited += POLL_INTERVAL;
        }
        boot::stall(POLL_INTERVAL);
    }
}

/// Voice the firmware boot screen through the native screen reader, on the visible
/// console, and let the user review it and continue by keyboard - all before the
/// kernel is loaded and boot services end. `width`/`height` are the display mode
/// the firmware reported, so the machine is described as it actually is.
///
/// Deterministic apart from the display line, and safe unattended: with nobody at
/// the keyboard it reads the screen and continues on its own. On an accessibility
/// invariant violation it emits `AW_UEFI_SR_FAIL` and withholds the proof marker
/// rather than claiming success, but it still lets the machine boot - stranding a
/// user at a dead firmware screen would be the worse failure.
pub fn run(width: usize, height: usize) {
    log::info!("AW_UEFI_SR_BEGIN");

    // Bring up the machine's real audio (HDA) once: each line is then spoken aloud
    // through it. On a thin laptop with no PC-speaker buzzer this codec is the only
    // thing that will actually sound; when there is no HDA controller, the PC
    // speaker plays a chime so at least the boot is audibly confirmed.
    let mut speaker = hda::bring_up();
    if speaker.is_none() {
        sound::startup_chime();
    }

    // A clean surface for the spoken screen: the boot log lives on the debug
    // console, so clearing here only affects what a person sees on the display.
    let _ = system::with_stdout(|stdout| stdout.clear());
    uefi::println!("Accessible Windows");
    uefi::println!();

    // The display line is built from the real GOP mode. It is the one line that
    // is not compile-time constant, so it is the one line the proofs do not pin.
    let display: String = alloc::format!("Display {width} by {height}");

    // What a user hears the instant the firmware hands control to us: the system
    // names itself, confirms the screen reader is already live, states the real
    // display mode, and narrates the one thing this stage does - load the OS.
    let screen = [
        node(1, Role::Window, "Accessible Windows"),
        node(2, Role::StaticText, "Screen reader active at firmware stage"),
        node(3, Role::StaticText, "Starting Accessible Windows"),
        node(4, Role::StaticText, &display),
        node(5, Role::StaticText, "Loading the operating system"),
    ];
    // Pre-recorded speech for each line, in order; the dynamic display line has no
    // clip and is spoken only on the console (until a runtime synthesizer lands).
    let clips: [Option<&'static [u8]>; 5] = [
        Some(hda::CLIP_WELCOME),
        Some(hda::CLIP_ACTIVE),
        Some(hda::CLIP_STARTING),
        None,
        Some(hda::CLIP_LOADING),
    ];

    for (line, clip) in screen.iter().zip(clips.iter()) {
        if !speak(line, *clip, &mut speaker) {
            // A constant node failed to validate: a bug, not a runtime condition.
            // Report it and skip the success marker, but keep booting.
            return;
        }
    }

    review_and_continue(&screen, &clips, &mut speaker);

    log::info!("AW_UEFI_SR_PROOF_OK");
}
