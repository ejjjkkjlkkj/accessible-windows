# Windows 11 accessible installation media — engineering track

## Objective

Prepare and validate Windows 11 x64 French installation media while keeping keyboard and screen-reader access as a first-class requirement. This track complements, rather than replaces, the native pre-OS UEFI reader.

## Baseline facts

- Windows Setup accessibility must be tested in the actual setup environment; having Narrator installed in the running Windows OS does **not** prove that speech works in WinPE/Windows Setup.
- An ISO rebuilt from UUP packages is not automatically an accessible ISO. UUP download/conversion, media integrity, Setup-time accessibility, and the custom UEFI reader are separate validation gates.
- Keep all work read-only until a candidate ISO is validated. Never write firmware/NVRAM, format USB media, or reboot automatically.
- Do not label QEMU/OVMF or VMware results as physical-hardware proof.

## Work sequence

1. **Inventory** — locate existing ISO/UUP/CAB/WIM/ESD/ARIA2 files and record sizes, timestamps, hashes, and available tools.
2. **Source validation** — select the exact Windows 11 build, fr-fr, x64, and required edition set. Prefer Microsoft-served UUP payloads and inspect generated scripts before execution.
3. **Build isolation** — download and convert in a new, dedicated working directory. Preserve existing source files and reports; do not overwrite a known ISO.
4. **ISO structural validation** — confirm UEFI boot files, boot.wim, and an install image (install.wim or install.esd); record SHA-256.
5. **Setup accessibility** — test from boot through language selection, disk-selection screens (without committing changes), error paths, and first-run setup. Record whether speech is audible, keyboard-only navigation works, focus is announced, and speech can be interrupted.
6. **Pre-OS reader integration** — validate the native UEFI screen-reader path separately from Windows Narrator. Chain-loading Windows Setup must not be called accessible until real speech and navigation are demonstrated.
7. **Evidence** — label every result PASS, FAIL, PARTIAL, BLOCKED, or NOT_VALIDATED; include build, machine/VM, commit, artifact hashes, and test operator. Human-confirmed audible speech is required for physical PASS.

## Immediate implementation

Run the read-only inventory script at scripts/audit-windows11-accessible-media.ps1.

It does not download files, mount an ISO, modify media, write firmware/NVRAM, format USB devices, or reboot. An ISO can be marked structurally validated only when an available 7-Zip-compatible executable can list its contents and the expected boot/install entries are found.

## Official references

- Microsoft: https://support.microsoft.com/fr-fr/accessibility/windows/use-a-screen-reader-to-set-up-windows
- Microsoft: https://support.microsoft.com/fr-fr/windows/cr%C3%A9er-un-support-d-installation-pour-windows-99a58364-8c02-206f-aa6f-40c3b507420d
- Microsoft: https://learn.microsoft.com/fr-fr/windows-hardware/get-started/adk-install
- UUP Dump project: https://github.com/UUP-Dump

## Current status

- Inventory script authored; execution on the user's Windows machine: NOT_VALIDATED.
- ISO rebuild: BLOCKED until source/build selection and available disk space are verified.
- Windows Setup speech/accessibility: NOT_VALIDATED until tested in the actual setup environment.
- Physical ASUS M1603QA proof: NOT_VALIDATED; no physical test is claimed by this document.
