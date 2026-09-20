//! Pluggable firmware-stage audio: one speaker interface over every self-built backend.
//!
//! UEFI defines no audio output protocol, so the project builds its own drivers rather
//! than depending on a vendor: [`crate::hda`] (Intel HDA) and [`crate::ac97`] (AC'97). This
//! picks whichever a machine actually has, so the same pre-recorded clips are spoken on any
//! codec; when neither is present the caller falls back to the PC speaker
//! ([`crate::sound`]), the universal last resort. New backends (USB Audio Class,
//! VirtIO-sound) slot in here without touching the screen reader or the setup.

use crate::ac97;
use crate::hda;

/// A brought-up audio output, whichever backend it is.
pub enum Speaker {
    Hda(hda::Speaker),
    Ac97(ac97::Speaker),
}

impl Speaker {
    /// Play a clip, stopping early if `interrupted` returns true (barge-in).
    pub fn speak_until(&mut self, clip: &[u8], interrupted: impl FnMut() -> bool) -> bool {
        match self {
            Speaker::Hda(speaker) => speaker.speak_until(clip, interrupted),
            Speaker::Ac97(speaker) => speaker.speak_until(clip, interrupted),
        }
    }

    /// Play a clip to completion, without barge-in.
    pub fn speak(&mut self, clip: &[u8]) -> bool {
        self.speak_until(clip, || false)
    }

    /// The backend's name, for the `AW_UEFI_AUDIO_BACKEND` marker and field logs.
    pub fn backend(&self) -> &'static str {
        match self {
            Speaker::Hda(_) => "hda",
            Speaker::Ac97(_) => "ac97",
        }
    }
}

/// Try each self-built audio backend in turn - HDA, then AC'97 - returning the first that
/// comes up. `None` means no codec was found, so the caller uses the PC speaker.
pub fn bring_up() -> Option<Speaker> {
    if let Some(speaker) = hda::bring_up() {
        return Some(Speaker::Hda(speaker));
    }
    if let Some(speaker) = ac97::bring_up() {
        return Some(Speaker::Ac97(speaker));
    }
    None
}
