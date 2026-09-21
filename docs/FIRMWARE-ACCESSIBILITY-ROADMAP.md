# Firmware-stage accessibility: what ships, and the roadmap

Accessible Windows speaks and is operable from the UEFI boot application, before the
kernel loads — see the "Firmware-stage screen reader" row in
[KERNEL-BOOT-PROOFS.md](KERNEL-BOOT-PROOFS.md). This note records what is implemented and
the gaps that remain, from a survey of the state of the art (no mainstream firmware
vendor ships a talking BIOS; only research prototypes and Apple's post-firmware VoiceOver
exist).

## Implemented

- **Universal, self-built audio** — UEFI has no standard audio protocol, so the project
  builds its own drivers rather than depending on any vendor, behind a pluggable backend
  (`boot/uefi/src/audio.rs`): **Intel HDA** and **AC'97** both stream pre-recorded PCM by
  DMA (Machado & Vieira, [arXiv:1712.03186](https://arxiv.org/abs/1712.03186), realized and
  taken past where they stopped at the codec beep), and the **PC speaker** is the universal
  last resort. The same clips are spoken on whatever a machine has; the active backend is
  reported (`AW_UEFI_AUDIO_BACKEND channel=hda|ac97|pc_speaker`).
- **A complete, tabbed Setup Utility with submenus**, modeled on AMI Aptio / the ASUS UEFI
  BIOS Utility (Main, Advanced → CPU Configuration, Boot → Boot Option Priorities →
  device, Security → Secure Boot, Save and Exit), spoken and keyboard-operable.
- **Real machine state, read and spoken**: SMBIOS system identity (manufacturer, product,
  serial, BIOS vendor/version/date — Type 0/1), firmware vendor/version, UEFI revision,
  RTC time, installed memory, display mode, CPU brand, virtualization support (CPUID +
  `IA32_FEATURE_CONTROL`), Secure Boot / Setup Mode, `BootCurrent`, `Timeout`.
- **Real boot actions**: enumerate `BootOrder`/`Boot####`; boot a device now (`BootNext`),
  make it the persistent default or move it up/down (`BootOrder`); enter the firmware's own
  setup (`OsIndications`); reset / shut down (`ResetSystem`).
- **Screen-reader affordances**: one consistent voice for all fixed scaffolding, an
  instructions clip on entry, repeat (Space), read-all (A), help (H/F1), where-am-I (W),
  first/last (Home/End), spell-by-character (S) from a synthesized A–Z/0–9 alphabet, and
  interruptible speech (barge-in).
- **COM1 serial mirror** so VMware and physical hardware capture the same `AW_UEFI_*`
  markers as QEMU's 0xE9 debug port.
- **Runtime speech synthesis** for arbitrary dynamic text — a from-scratch Klatt-style
  cascade formant synthesizer (`boot/uefi/src/synth.rs`), so the enumerated boot-device
  names, CPU brand, memory sizes, resolutions and firmware setting values are spoken as
  *words*, not just spelled. English letter-to-sound rules drive the word path, all-caps
  tokens spell as letters, and numbers are read in words (English and French). It emits the
  same 24 kHz mono PCM the codecs already stream, so nothing new sits below it; the speech
  rate and pitch are adjustable live (`[`/`]`, `,`/`.`). Proven at boot on OVMF
  (`AW_UEFI_SYNTH_SELFTEST`). Honest scope: intelligible and robotic, like early DECtalk —
  the right trade for understanding a value you otherwise could not hear at all.
- **TPM and Secure Boot key state** — the TCG2 TPM presence/PCR-bank state and the PK/KEK/db/dbx
  certificate counts are read and spoken (Security submenu and agent), proven headless as
  `AW_UEFI_SECURITY`.

## Roadmap (surveyed gaps, not yet implemented)

- **Audio hardware coverage beyond HDA and AC'97**: USB Audio Class (thin laptops /
  dongles, needs a USB host stack) and VirtIO-sound (VMs) are the remaining backends to
  build into `audio.rs`. Tracks the still-unstandardized UEFI audio work (no audio output
  protocol in the UEFI spec as of 2.11, Dec 2024; see the GSoC effort and
  [tait.tech/blog/uefi-audio](https://tait.tech/blog/uefi-audio/)).
- **HII integration** — voice the firmware's *own* setup forms via the Human Interface
  Infrastructure ([UEFI 2.11 ch. 33](https://uefi.org/specs/UEFI/2.11/33_Human_Interface_Infrastructure.html)),
  so settings only the firmware owns (SATA mode, XMP, CSM, passwords, TPM) become spoken.
- **Pre-boot braille** via a USB HID Braille display
  ([HUTRR78](https://usb.org/sites/default/files/hutrr78_-_creation_of_a_braille_display_usage_page_0.pdf));
  `aw-braille` already renders cells at the kernel stage. BRLTTY is post-kernel only.
- **More screen-reader depth**: an independent review cursor and read-by-line across the whole
  screen still to come. (Done: adjustable rate/volume/pitch, phonetic Alpha/Bravo spelling,
  cycled verbosity levels (`V`), punctuation levels (`X`), and read-by-word of the focused line
  (`O`) - all proven driven from the keyboard under QEMU.)
- **More real UEFI settings**: *done* — setting the RTC clock (raw `SetTime` from the agent,
  "set time 14:30" / "set date 2026-09-21"), the `Driver####`/`SysPrep####` load lists (read
  and spoken, `AW_UEFI_LOADOPTS`), and richer Secure Boot key/certificate state (PK/KEK/db/dbx)
  and TPM presence.

## Standards framing

EN 301 549 and Section 508 require **self-voicing** for *closed functionality* — systems
that do not permit assistive technology to attach, which a BIOS/UEFI setup is. A firmware
setup that speaks itself is exactly what those standards call for.
