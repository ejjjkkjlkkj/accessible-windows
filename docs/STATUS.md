# Status

Updated: 2026-09-24

## Release candidate

- Repository: `ejjjkkjlkkj/accessible-windows`
- Branch: `release/microsoft-preview-20260924`
- Source line: `uefi-screenreader-live-integration-v2-20260922`
- Consolidation source head: `238c9dd4b22662c1b92314548a784a934504c45d`
- Release intent: Microsoft technical review preview
- Production readiness: **not claimed**

This branch is intentionally curated. It excludes historical labs, superseded CI files and unrelated experiments from the published release tree.

## Software validation contract

The exact release commit must pass `.github/workflows/release-validation.yml`.

Required evidence produced by that workflow:

- freestanding core compilation with `-Werror`;
- AddressSanitizer and UndefinedBehaviorSanitizer behavior tests;
- Clang static analysis;
- x86-64 EFI link of the V2 core;
- OVMF runtime smoke markers ending in `STATUS=PASS`;
- HDA V2 speech-unit generation and EFI link;
- deterministic 96 MiB GPT/FAT32 USB image creation;
- SHA-256 manifests;
- uploaded release-candidate artifact bundle.

A failed, cancelled, skipped or stale run is not PASS.

## Physical baseline kept separate

GitHub Actions run `35846636674` completed successfully on 2026-09-23 on the Windows self-hosted runner. That run exercised a pinned external project payload at commit `8302a95b0b4d2fe4d57e2832bcc945819728b80d` and recorded:

- `PSEXEC_INTERACTIVE_SYSTEM=PASS`
- `PSEXEC_REQUIRED=PASS`
- `IDENTITY_SID=S-1-5-18`
- `REAL_WINDOWS_VOICE_BUILD=PASS`
- `VOICE_CODEC_ROUNDTRIP=PASS`
- `NAVIGATION_EFI_REAL_VOICE_BUILD=PASS`
- `VMWARE_UEFI_BOOT=PASS`
- `VMWARE_SCREENREADER_DISCOVERY=PASS`
- `SYSTEM_SPEECH_OUTPUT_RATE=16000`

That evidence demonstrates the prior physical baseline only. It does not automatically validate a later release commit.

## Open technical closure

The preview does not claim complete screen-reader parity. Open work includes broader HII control/action semantics, form hierarchy, safe editing/activation, stronger punctuation/value preservation, speech cancellation behavior, larger prompt capacity, more OEM hardware coverage, and human-confirmed intelligibility on the final target path.
