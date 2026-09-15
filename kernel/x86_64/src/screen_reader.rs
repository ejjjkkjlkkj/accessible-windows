//! Native screen-reader announcement proof (roadmap Phase 5 "Native screen
//! reader"; the project's accessibility-first contract).
//!
//! Every other proof in this kernel shows the machine doing something; this one
//! shows the machine *saying* something. It builds the semantic tree of the first
//! screen a user meets - the installer's welcome dialog - the way an accessible
//! UI toolkit would expose it, validates each node against the accessibility
//! invariants in [`aw_accessibility`], and walks it in focus order, emitting the
//! exact utterance the [`aw_screen_reader`] engine would hand a speech or braille
//! device for each control. It also toggles the "enable screen reader" check box
//! and speaks the resulting state-change event.
//!
//! No display, speech device or keyboard is required: the utterances are emitted
//! on the debug channel as `AW_SR_SPEAK "..."`, which is the nonvisual delivery
//! evidence the accessibility contract asks for - proof that a blind user would
//! be told what each control is, in words, this early in boot. The wording is
//! unit-tested in the `aw-screen-reader` crate; here it is proven to run,
//! unchanged, inside the real kernel.

use aw_accessibility::{validate_node, NodeId, Rect, Role, SemanticNode, State};
use aw_screen_reader::{announce_event, announce_focus, FocusContext};

use crate::debug_write;

/// Build one semantic node of the sample installer dialog.
fn node(
    id: u64,
    role: Role,
    name: &'static str,
    value: &'static str,
    description: &'static str,
    state_bits: u32,
) -> SemanticNode<'static> {
    SemanticNode {
        id: NodeId(id),
        parent: Some(NodeId(0)),
        role,
        name,
        description,
        value,
        state: State::from_bits(state_bits),
        // A plausible on-screen rectangle: interactive controls must be non-zero
        // sized to satisfy the accessibility invariants.
        bounds: Rect {
            x: 40,
            y: 40,
            width: 320,
            height: 28,
        },
    }
}

/// Emit the focus utterance for one node, or fail the proof if the node violates
/// an accessibility invariant. Returns false on failure.
fn speak_focus(node: &SemanticNode<'_>, context: FocusContext) -> bool {
    if validate_node(node).is_err() {
        debug_write("AW_SR_FAIL reason=invalid_node\n");
        return false;
    }
    let mut buffer = [0u8; 256];
    let text = announce_focus(node, context, &mut buffer);
    if text.is_empty() {
        debug_write("AW_SR_FAIL reason=empty_utterance\n");
        return false;
    }
    debug_write("AW_SR_SPEAK \"");
    debug_write(text);
    debug_write("\"\n");
    true
}

/// Prove the native screen reader speaks the installer's first screen: build its
/// accessible tree, validate it, and voice every control in focus order plus one
/// state-change event. Deterministic and device-free, so it runs on the normal
/// boot path.
pub fn prove() {
    debug_write("AW_SR_BEGIN\n");

    // The welcome dialog, in the order the keyboard focus visits it. A blind user
    // tabbing through hears exactly these lines.
    let dialog = node(
        1,
        Role::Dialog,
        "Install Accessible Windows",
        "",
        "",
        0,
    );
    let welcome = node(
        2,
        Role::StaticText,
        "Welcome to Accessible Windows setup",
        "",
        "",
        0,
    );
    let language = node(
        3,
        Role::ComboBox,
        "Language",
        "English",
        "",
        State::FOCUSABLE,
    );
    let mut screen_reader_toggle = node(
        4,
        Role::CheckBox,
        "Enable screen reader at boot",
        "",
        "",
        State::FOCUSABLE | State::CHECKED,
    );
    let speech_rate = node(
        5,
        Role::Slider,
        "Speech rate",
        "40%",
        "",
        State::FOCUSABLE,
    );
    let install = node(
        6,
        Role::Button,
        "Install",
        "",
        "installs to the selected disk",
        State::FOCUSABLE,
    );
    let recovery = node(
        7,
        Role::Button,
        "Recovery options",
        "",
        "",
        State::FOCUSABLE,
    );

    // Entering the dialog announces the container itself, then focus order.
    let ok = speak_focus(&dialog, FocusContext::NONE)
        && speak_focus(&welcome, FocusContext::NONE)
        && speak_focus(&language, FocusContext::NONE)
        && speak_focus(&screen_reader_toggle, FocusContext::NONE)
        && speak_focus(&speech_rate, FocusContext::NONE)
        // The two buttons form a set the user can hear their way through.
        && speak_focus(&install, FocusContext::in_set(1, 2))
        && speak_focus(&recovery, FocusContext::in_set(2, 2));
    if !ok {
        return;
    }

    // Toggling the check box: the screen reader voices the state change alone, not
    // the whole control, the way it does when the user presses Space.
    screen_reader_toggle.state = State::from_bits(State::FOCUSABLE);
    let mut buffer = [0u8; 64];
    let event = announce_event(
        aw_accessibility::AccessibilityEvent::StateChanged(screen_reader_toggle.id),
        &screen_reader_toggle,
        &mut buffer,
    );
    if event.is_empty() {
        debug_write("AW_SR_FAIL reason=empty_event\n");
        return;
    }
    debug_write("AW_SR_EVENT \"");
    debug_write(event);
    debug_write("\"\n");

    debug_write("AW_SR_PROOF_OK\n");
}
