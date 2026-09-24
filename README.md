# Accessible Windows — UEFI Accessibility Preview

Accessible Windows is an experimental, accessibility-first UEFI project. The current release line focuses on a native pre-OS screen-reader core that can discover HII/IFR controls, maintain semantic focus, format spoken state, and integrate with a UEFI HDA speech path.

This repository is a research preview, not a production firmware replacement and not a claim of full Windows screen-reader parity.

## Release scope

The Microsoft review preview contains only the canonical implementation and validation material:

- `boot/uefi-screenreader-core-v1/` — bounded semantic screen-reader core and live HII adapter.
- `boot/uefi-hii-graph-prompt-speech-v1/` — retained golden HII/HDA reference path.
- `boot/uefi-hii-graph-prompt-speech-v2/` — V2 integration target.
- `boot/uefi-native-speech-v1/build_uefi_native_speech.py` — deterministic first-party speech-unit source required by the V2 generator.
- `scripts/build-uefi-screenreader-usb-image.sh` — deterministic x86-64 UEFI USB image builder.
- `.github/workflows/release-validation.yml` — exact-commit software validation and release artifact build.
- `docs/` — status, release notes, security boundary and Microsoft review notes.

Historical labs, superseded workflows and unrelated OS experiments are intentionally excluded from this release branch. They remain available in repository history and archival branches.

## Implemented capabilities

- bounded HII/IFR parsing with malformed-length rejection;
- stable semantic control identity derived from firmware identifiers;
- focus navigation, role navigation, paging and searchable item chooser;
- state/value speech formatting with password-value redaction;
- bounded speech queue with duplicate suppression, priority and preemption;
- live HII snapshot refresh with transactional publication;
- freestanding x86-64 UEFI build;
- QEMU/OVMF runtime smoke validation for the core;
- HDA V2 link validation;
- deterministic USB image generation with SHA-256 output.

## Validation

The release candidate is valid only if `release-validation` succeeds for the exact commit being reviewed. The workflow compiles with warnings-as-errors, runs ASan/UBSan behavior tests, runs Clang static analysis, boots the core under OVMF, links the HDA V2 firmware, builds the USB image and uploads the evidence bundle.

The most recent separately recorded physical Windows/VMware baseline is documented in `docs/STATUS.md`. Physical evidence is kept distinct from hosted software validation.

## Known limitations

Full production parity is not claimed. Remaining work includes OEM-specific physical UEFI coverage, broader real-HII control semantics, human-confirmed speech quality on target hardware, safe state-changing actions, and additional recovery/pre-OS integration.

See `docs/MICROSOFT_REVIEW.md` and `docs/STATUS.md`.
