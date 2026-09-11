# Accessible Windows

Accessible Windows is an accessibility-first desktop operating-system project.

The project is **not a modified copy of Microsoft Windows, Windows 10X, or Windows Core OS**. It is an original open architecture intended to provide a familiar Windows-class desktop while integrating Windows, Linux, Android, and progressively macOS-compatible applications behind one accessible user experience.

## Core goals

- Accessibility is a platform service, not an optional application.
- Screen-reader access must start at the earliest practical boot/recovery stage.
- Keyboard-only operation must cover installation, recovery, login, desktop, settings, application launch, updates, and shutdown.
- The UI and platform must expose stable semantic accessibility trees.
- Windows, Linux, Android, and supported macOS applications should appear in one launcher/task model.
- Prefer native or compatibility-layer execution; use virtualization only when isolation or compatibility requires it.
- Target a maintainable 10-year platform lifecycle with replaceable components and reproducible builds.
- x86-64 is the initial hardware target; ARM64 is a planned architecture, not an afterthought.

## Architecture baseline

```text
UEFI / Secure Boot
        |
Accessible boot + recovery environment
        |
Linux LTS kernel + minimal host userspace
        |
Accessible platform services
  |-- accessibility broker
  |-- speech / braille broker
  |-- input and shortcut service
  |-- settings / identity / permissions
  |-- package and update service
  `-- app integration broker
        |
        +-- Native Linux applications
        +-- Windows applications via Wine/Win32 compatibility
        +-- Android system/container integration
        +-- macOS compatibility via Darling-class runtime where technically/legalistically possible
        `-- QEMU/KVM fallback for workloads requiring a guest OS
        |
Unified accessible desktop shell
```

## External projects are components/references, not the product

The project may integrate, adapt, or study mature open-source technology such as:

- Linux LTS kernel
- Wine / vkd3d for Win32/Direct3D compatibility
- Waydroid-style Android container integration
- Darling for Darwin/macOS compatibility research
- QEMU/KVM for virtualized fallback workloads
- systemd-boot or GRUB-class UEFI boot management
- ReactOS as a Windows-compatibility research/reference project

Every imported component must remain replaceable behind an Accessible Windows interface. No third-party project becomes the architecture owner.

## Non-negotiable accessibility requirements

1. No mouse-only critical path.
2. No silent visual-only error state.
3. Focus is always programmatically discoverable.
4. Text, caret, selection, role, state, name, description, value, and actions are exposed semantically.
5. Password/secret fields are protected from speech and logging by policy.
6. Installer and recovery environments are screen-reader usable.
7. Compatibility layers must bridge their accessibility APIs into the host accessibility model.
8. Automated accessibility tests and real screen-reader/keyboard validation are release gates.

## Current status

The repository was initialized on 2026-09-11. The first milestone is **Architecture + Bootable Accessible Baseline**, before attempting broad application compatibility.

See:

- `docs/architecture.md`
- `docs/accessibility.md`
- `docs/compatibility.md`
- `docs/boot-recovery.md`
- `docs/roadmap.md`

## Development rule

A feature is not considered complete when it only renders correctly. It is complete when it can be discovered, understood, controlled, and recovered from without sight.
