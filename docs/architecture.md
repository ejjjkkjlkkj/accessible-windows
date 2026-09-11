# Architecture

## 1. Design objective

Accessible Windows is an original accessibility-first desktop platform. Its architecture must survive component replacement over a long support horizon and must not depend on proprietary Microsoft Windows internals.

The host platform is Linux-based because it provides an open kernel, mature hardware enablement, namespaces/cgroups, KVM, broad filesystem/network support, and the primitives required to host multiple compatibility domains.

## 2. Architectural layers

### Layer A — Firmware and trusted boot

- UEFI x86-64 first; ARM64 planned.
- Secure Boot compatible boot chain.
- A/B bootable system generations where practical.
- Recovery entry available independently of the main desktop.
- Boot state and failures represented both visually and through accessible audible/haptic signaling where firmware permits.

### Layer B — Host kernel and minimal userspace

Responsibilities:

- CPU, memory, storage, network, USB, Bluetooth, audio, graphics, input.
- KVM virtualization.
- namespaces/cgroups for application domains.
- mandatory access-control hooks.
- suspend/resume and power management.

The host must stay intentionally small. Desktop policy belongs above this layer.

### Layer C — Platform services

Long-lived services expose stable IPC contracts so implementation components can be replaced.

Proposed services:

- `aw-accessibilityd` — semantic accessibility graph and event broker.
- `aw-speechd` — speech synthesis broker.
- `aw-brailled` — braille display routing and input.
- `aw-inputd` — global shortcuts, keyboard routing, alternate input.
- `aw-sessiond` — session/login lifecycle.
- `aw-appd` — application discovery, launch, lifecycle, domain selection.
- `aw-packaged` — package metadata and installs.
- `aw-updated` — atomic update orchestration and rollback.
- `aw-portald` — files, clipboard, notifications, camera, microphone, secrets, URI and picker portals.
- `aw-compatd` — compatibility-domain coordination.
- `aw-recoveryd` — diagnostics/recovery orchestration.

Services should expose versioned D-Bus or similarly inspectable IPC initially, with a path to a Rust-native protocol where performance requires it.

## 3. Accessibility graph

All UI domains map into one normalized semantic object model.

Minimum node properties:

- stable runtime ID
- role
- name
- description
- value
- states
- bounds
- parent/children
- relations
- actions
- text interfaces
- caret and selection
- live-region/event priority
- sensitivity/secret classification
- source domain
- source process/application

Core events:

- focus changed
- object created/destroyed
- name/value/state changed
- text inserted/removed/replaced
- caret moved
- selection changed
- live-region changed
- notification announced
- window activated/deactivated

The graph is the contract consumed by the built-in screen reader and can also be exposed to third-party assistive technologies.

## 4. Desktop shell

The shell must be designed around semantics before pixels.

Required surfaces:

- login
- desktop/workspace
- launcher/search
- task switcher
- quick settings
- notification center
- settings
- file picker
- file manager
- terminal
- package/application center
- shutdown/restart/recovery UI

Every control must have a deterministic keyboard path and semantic representation.

## 5. Application domains

### Native Linux

Preferred protocol stack:

- Wayland compositor
- PipeWire
- xdg-desktop-portal model
- AT-SPI bridge into `aw-accessibilityd`

X11 should be compatibility-only where required.

### Windows

Primary path:

- Wine/Win32 compatibility
- vkd3d/Direct3D translation when needed
- per-application prefixes/containers
- UI Automation/MSAA/IAccessible2 bridge into the host accessibility graph

Fallback path:

- isolated Windows VM through QEMU/KVM when an application cannot run safely or correctly in the compatibility layer
- integrated launcher, clipboard/file portals, notifications, audio, and accessible remote UI bridge where licensing permits

Windows kernel drivers are not treated as normal user applications and are not loaded into the host kernel.

### Android

Primary path:

- full Android userspace in an isolated Linux container
- binder/binderfs integration
- graphics/audio/input integration
- Android AccessibilityNodeInfo/events mapped into `aw-accessibilityd`
- application activities exported into the unified launcher

The Android runtime is a platform domain, not a separate desktop window by default.

### macOS/Darwin compatibility

Primary research path:

- Darling-class Mach-O/Darwin compatibility runtime
- reimplemented open frameworks where legally distributable
- native integration broker for windows, files, clipboard, notifications, audio and accessibility

This domain is explicitly capability-driven. Unsupported proprietary Apple frameworks must not be copied or redistributed. Applications requiring unavailable proprietary components may remain unsupported.

## 6. Virtualization

QEMU/KVM is the controlled fallback, not the first choice.

Use cases:

- applications requiring a guest kernel
- high-risk compatibility workloads
- regression testing
- OS compatibility validation
- recovery/testing sandboxes

Guest windows should eventually be surface-forwarded into the host shell rather than presented only as an opaque VM desktop.

## 7. Storage and updates

Target model:

- EFI System Partition
- immutable or transactionally updated system image
- separate writable state/data
- recovery image
- A/B or generation-based rollback

Update properties:

- signed metadata and payloads
- reproducible build provenance
- staged deployment
- health check after boot
- automatic rollback on failed boot or inaccessible login path
- accessibility regression gate before stable promotion

## 8. Security model

Principles:

- least privilege
- sandbox by default
- compatibility domains separated from host services
- portals for privileged resources
- no unrestricted cross-domain clipboard/files/device access without policy
- secrets never exposed through accessibility APIs
- assistive technology gets controlled semantic access, not arbitrary memory access

## 9. Ten-year maintainability

The platform must avoid tight coupling to individual upstream projects.

Rules:

- every major external component behind an internal interface
- exact source revision recorded for releases
- SBOM for images
- reproducible CI builds
- migration tests for stored settings/data
- LTS release branch plus continuously integrated development branch
- hardware enablement decoupled from shell/application releases where possible
- test matrices for x86-64 and ARM64 before ARM64 becomes production

## 10. First engineering milestone

A milestone is reached only when a bootable x86-64 image can:

1. boot under UEFI in QEMU/KVM;
2. start an accessible recovery/installer surface;
3. produce speech without requiring a graphical setup step;
4. accept keyboard-only navigation;
5. launch the minimal desktop shell;
6. expose the shell through the normalized accessibility graph;
7. survive update/rollback tests;
8. emit machine-readable validation results.
