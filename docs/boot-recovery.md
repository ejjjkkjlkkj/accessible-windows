# Boot and Recovery Design

## Objective

The machine must remain operable without sight even when the normal desktop cannot start. Recovery is a first-class system mode, not a hidden maintenance shell.

## 1. Disk layout baseline

Initial GPT layout:

1. EFI System Partition (`ESP`)
2. system slot A
3. system slot B or alternate generation store
4. persistent state/data
5. recovery image/state

Exact sizes remain build-profile dependent.

## 2. Boot manager

Requirements:

- UEFI native;
- Secure Boot compatible;
- deterministic keyboard navigation;
- entries for normal boot, previous known-good generation, recovery, firmware setup where supported;
- configurable timeout that can be cancelled from keyboard;
- no destructive action from the boot menu;
- machine-readable boot-attempt counters.

`systemd-boot` is a strong initial candidate because of its small UEFI-focused design. GRUB remains a compatibility candidate for hardware/configurations requiring its broader filesystem/boot support. The final platform interface must not depend on either boot loader's internal configuration format.

## 3. Unified Kernel Image direction

Where practical, each system generation should be represented as a signed Unified Kernel Image or equivalent signed boot artifact containing:

- kernel
- initramfs
- command line
- OS release metadata
- integrity metadata

Benefits:

- simpler verification;
- atomic generation selection;
- easier rollback;
- fewer mutable boot files.

## 4. Initramfs responsibilities

The early userspace should contain only what is required to discover and mount the system safely and to enter recovery.

Required capabilities:

- storage discovery;
- encrypted-volume unlock path;
- keyboard input;
- common USB stack;
- common audio path for local speech when feasible;
- network recovery as an optional capability;
- system-slot health checks;
- recovery shell/service launch;
- logging to persistent or exportable storage.

## 5. Accessible recovery UI

Recovery should expose a structured menu backed by `aw-recoveryd`.

Initial operations:

- continue normal boot;
- boot previous known-good system;
- run filesystem/storage checks;
- inspect hardware summary;
- inspect boot/update failure reason;
- repair boot entries;
- restore system generation;
- network diagnostics;
- export logs to removable media;
- open advanced terminal;
- reinstall system while preserving user data when possible;
- factory reset only behind explicit multi-step confirmation.

Every action exposes:

- name;
- description;
- risk level;
- target device/system generation;
- progress;
- success/failure state;
- recovery suggestion after failure.

## 6. Speech in recovery

Local speech is required; cloud speech is never a recovery dependency.

Strategy:

- ship a compact offline synthesizer and minimal language set in recovery;
- allow a documented shortcut to start/restart speech;
- remember preferred voice/audio device in persistent state when safe;
- fall back to a predictable default device;
- provide short audio status codes when full speech cannot initialize.

## 7. Braille in recovery

Recovery should eventually include a constrained braille stack for common USB HID and supported serial/USB devices. This is a staged goal because the hardware matrix is larger than the speech path.

## 8. Update transaction model

A system update must not replace the only bootable system in place.

Target flow:

1. download signed metadata/payload;
2. verify signature and hashes;
3. install into inactive slot/new generation;
4. validate image metadata and critical files;
5. create boot entry;
6. boot once in probation mode;
7. validate system, accessibility broker, speech, shell and login readiness;
8. mark generation healthy;
9. otherwise automatically return to previous known-good generation.

## 9. Accessibility health check

A boot is not considered healthy merely because PID 1 and the compositor are running.

Minimum health signals:

- input service alive;
- accessibility broker alive;
- speech broker alive or explicitly disabled by user policy;
- shell publishes semantic root;
- keyboard focus exists;
- login/session surface reachable;
- no fatal accessibility event-loop backlog;
- recovery shortcut/service reachable.

Failure of these checks can trigger rollback or a recovery prompt.

## 10. Test matrix

Automated QEMU tests should cover:

- normal UEFI boot;
- Secure Boot test configuration;
- missing/corrupt active slot;
- interrupted update;
- invalid new generation signature;
- inaccessible shell health check;
- no network;
- no GPU acceleration;
- alternate audio devices;
- disk-full state;
- encrypted system/data unlock path;
- rollback after failed probation boot.

A headless test mode must expose recovery/menu state over a machine-readable channel so CI can validate behavior without OCR.
