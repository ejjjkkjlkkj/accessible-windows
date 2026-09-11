# Roadmap

## Phase 0 — Foundation

Status: in progress.

Deliverables:

- architecture baseline;
- accessibility contract;
- boot/recovery design;
- multi-domain compatibility design;
- Rust workspace;
- normalized accessibility core model;
- CI on Linux/Windows/macOS for portable core crates;
- exact upstream component manifest and license inventory.

Exit criteria:

- repository builds and tests cleanly;
- accessibility object/event model has unit tests;
- project boundaries and unsupported assumptions are documented.

## Phase 1 — Bootable accessible baseline

Goal: produce the first x86-64 UEFI image usable without sight.

Deliverables:

- reproducible Linux host image builder;
- UEFI boot entry;
- initramfs;
- local offline TTS;
- keyboard-driven recovery shell/UI;
- minimal compositor/session;
- `aw-accessibilityd` prototype;
- `aw-speechd` prototype;
- minimal semantic desktop shell;
- QEMU automated boot tests.

Required real tests:

- start VM with no graphical interaction;
- trigger speech from keyboard;
- navigate recovery menus;
- reach login/session;
- launch terminal;
- shutdown/reboot;
- capture machine-readable accessibility events.

## Phase 2 — Native desktop completion

Deliverables:

- accessible launcher/search;
- task switcher;
- notifications;
- quick settings;
- settings application;
- accessible file manager;
- package center;
- Wi-Fi/Bluetooth UI;
- audio device selection;
- power/battery management;
- update UI with rollback state;
- braille prototype.

Exit criteria:

A blind user can install, configure, update, recover, and perform core desktop tasks without sighted assistance.

## Phase 3 — Windows application domain

Deliverables:

- managed Wine runtime;
- per-application prefixes;
- app manifest generation;
- file/URI association bridge;
- clipboard/notification/audio integration;
- UIA/MSAA/IAccessible2 accessibility bridge;
- Direct3D translation stack;
- compatibility database and regression harness;
- VM fallback path for applications requiring a Windows kernel.

Priority validation applications:

- Notepad-class text editor;
- browser;
- office/document editor;
- terminal/PowerShell-class console application;
- installer/MSI application;
- application exposing UIA TextPattern/TextPattern2;
- application exposing IAccessible2.

## Phase 4 — Android application domain

Deliverables:

- managed full Android container image;
- binder/binderfs integration;
- graphics/audio/input integration;
- activity registration in unified launcher;
- notifications/clipboard/files/camera/microphone portals;
- Android accessibility tree/event bridge;
- TalkBack-equivalent comparison tests;
- Android 16+ image track at implementation time.

Exit criteria:

Android applications are launched and switched like host applications, with host screen-reader access to meaningful semantic content.

## Phase 5 — macOS/Darwin compatibility domain

Deliverables:

- Darling-class runtime integration;
- `.app` bundle discovery;
- Mach-O launch service;
- window/audio/clipboard/file integration;
- accessibility semantics bridge where supported;
- application-by-application capability database.

Exit criteria:

Supported applications integrate cleanly. Unsupported proprietary framework dependencies are reported precisely rather than hidden behind generic failures.

## Phase 6 — Application virtualization integration

Deliverables:

- QEMU/KVM service integration;
- managed guest images;
- snapshots;
- virtio integration;
- clipboard/files/audio/notifications portals;
- application-window remoting research;
- guest accessibility semantic bridge research.

## Phase 7 — Hardware and architecture expansion

Deliverables:

- broad x86-64 hardware matrix;
- AMD/Intel integrated graphics validation;
- NVIDIA strategy based on legally redistributable drivers/packages;
- Wi-Fi/Bluetooth matrix;
- audio matrix;
- suspend/resume;
- ARM64 build and boot;
- ARM64 compatibility-domain strategy.

## Phase 8 — Long-term release engineering

Deliverables:

- signed stable releases;
- SBOM;
- reproducible builds;
- update channels;
- LTS branch policy;
- security response process;
- 10-year migration/update strategy;
- compatibility and accessibility telemetry that is opt-in and privacy-preserving;
- disaster-recovery and offline reinstall media.

## Release gates

Every stable release must pass:

1. clean reproducible build;
2. boot/recovery regression suite;
3. keyboard-only critical path;
4. speech-only critical path;
5. secret-field leakage tests;
6. accessibility graph consistency tests;
7. update + rollback tests;
8. compatibility-domain smoke tests;
9. real screen-reader workflow tests;
10. license/SBOM validation.

## Immediate next implementation order

1. Rust workspace and accessibility core model.
2. `aw-accessibilityd` in-memory graph prototype.
3. event subscription API and deterministic tests.
4. `aw-speechd` interface + mock synthesizer.
5. headless semantic shell prototype.
6. QEMU boot-image builder.
7. offline TTS in initramfs/recovery.
8. first blind-user end-to-end boot test.
