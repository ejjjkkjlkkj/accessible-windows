# Accessible Windows

Accessible Windows is an experimental low-level accessibility project focused on native UEFI and pre-OS screen-reader capabilities.

## Scope

The repository contains the firmware-side accessibility work: HII/IFR discovery, semantic navigation, native speech/audio paths, keyboard interaction, QEMU/OVMF validation, VMware validation, and physical AMD/ASUS proof gates.

The project does **not** claim complete screen-reader parity yet. The remaining parity requirements are tracked in issue #4.

## Canonical development line

The clean consolidation line is based on:

- `uefi-screenreader-live-integration-v2-20260922`
- base commit: `238c9dd4b22662c1b92314548a784a934504c45d`

This line is 18 commits ahead of the earlier `uefi-screenreader-core-v2-20260922` baseline and contains the live HII adapter plus HDA integration work.

Historical experiment branches are retained as evidence until their unique results are either integrated or explicitly archived.

## Repository layout

- `boot/` — UEFI boot and screen-reader implementations.
- `system/` — native system/accessibility experiments.
- `lab*/` — staged low-level proofs and regressions.
- `scripts/` — build and validation helpers.
- `.github/workflows/` — CI, VM, firmware and hardware proof gates.
- `docs/STATUS.md` — current verified state and unresolved gates.

## Validation policy

A result is marked PASS only when its workflow or physical evidence contains the required proof markers. Software simulation, VMware execution and physical-hardware evidence are kept distinct.

For the Windows physical runner, privileged local execution must use:

`C:\Users\adm\Downloads\PsExec64.exe`

with LocalSystem identity validation (`S-1-5-18`) when required by the hardware pipeline.

## Current physical baseline

The last known green physical PsExec/VMware baseline used project commit:

`8302a95b0b4d2fe4d57e2832bcc945819728b80d`

That run proved the baseline voice build, codec round-trip, NAVIGATION.EFI build, VMware UEFI boot and screen-reader discovery. It produced speech at 16 kHz. The later 24 kHz voice target remains a separate unclosed gate.

See `docs/STATUS.md` for exact evidence and blockers.
