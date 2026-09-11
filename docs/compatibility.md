# Application Compatibility Strategy

## Goal

Applications from several ecosystems should be discoverable and launched from one accessible desktop. The user should not need to understand whether an application is native, translated, containerized, or virtualized for normal daily use.

Compatibility is implemented as domains behind `aw-compatd` and `aw-appd`.

## 1. Common application contract

Each installed application is represented by a normalized manifest containing:

- application ID
- display name
- icon/resource references
- source domain
- architecture
- launch target
- file/URI associations
- requested permissions
- accessibility capability level
- preferred sandbox profile
- update source
- compatibility/runtime version

The launcher consumes this manifest rather than domain-specific shortcuts.

## 2. Windows domain

### Primary execution path

Use Wine-class Win32 translation rather than a VM whenever practical.

Required integration work:

- per-app Wine prefixes
- automatic architecture selection
- Win32 process supervision
- Wayland-native window integration where available
- Direct3D through vkd3d/DXVK-class translation as appropriate
- audio through host PipeWire integration
- file/URI portals
- host notifications
- system tray bridging
- MIME/file association bridging
- clipboard policy
- input method integration
- UIA/MSAA/IAccessible2 accessibility bridge

### Compatibility policy

Applications are classified:

- `native-quality`: suitable for normal use through translation;
- `compatible`: works with known limitations;
- `isolated-required`: must run in a VM/sandbox for correctness or security;
- `unsupported`: cannot currently meet functional/accessibility requirements.

### Drivers

Windows kernel-mode drivers are not loaded into the host Linux kernel. Hardware support must come from host drivers. Applications depending on proprietary kernel drivers may require a Windows guest.

## 3. Linux domain

Native Linux applications are first-class applications.

Preferred stack:

- Wayland
- PipeWire
- xdg-desktop-portal style permission mediation
- AT-SPI bridge
- Flatpak-like sandboxing concepts where useful, without making a single packaging format mandatory

Legacy X11 applications can be hosted through XWayland until replaced or migrated.

## 4. Android domain

A full Android userspace runs in a managed container using Linux kernel primitives.

Required integration:

- binder/binderfs
- namespaces/cgroups
- graphics buffer transport
- audio
- input
- network
- sensors where available
- notifications
- clipboard policy
- files/media portals
- camera/microphone portals
- Android activity export into the host launcher
- accessibility event/tree bridge

The design should support modern Android images rather than permanently pinning to an obsolete Android generation.

Google Play services are not assumed to be redistributable by this project. The architecture must support legal user-provided or licensed service packages without making them a hard dependency of the base OS.

## 5. macOS/Darwin-compatible domain

The project may use Darling-class technology and open Darwin components to execute supported Mach-O applications on Linux.

Required integration targets:

- process launch
- Mach-O loader/runtime
- filesystem mapping
- window/surface forwarding
- clipboard
- audio
- notifications
- keyboard/input
- application bundle registration
- accessibility semantics where the compatibility runtime exposes them

Limitations are explicit:

- proprietary Apple frameworks cannot simply be copied into the distribution;
- hardware/DRM-dependent applications may remain unsupported;
- applications tied to unsupported AppKit/Metal/framework behavior may fail;
- compatibility must be measured application by application.

The shell must never mislabel partial macOS compatibility as complete macOS support.

## 6. Virtual-machine fallback

QEMU/KVM provides a fallback domain for applications requiring a real guest kernel.

The long-term UX target is application remoting, not a mandatory full guest desktop window.

Integration targets:

- lifecycle controlled by `aw-appd`;
- snapshot/rollback;
- virtio devices;
- shared-file portal rather than unrestricted host mounts;
- clipboard mediation;
- audio routing;
- notification forwarding;
- optional GPU acceleration;
- accessibility bridge from guest semantics when technically possible.

Guest operating-system licenses remain the user's/project distributor's responsibility.

## 7. Architecture translation

Initial host: x86-64.

Future ARM64 host strategy:

- native ARM64 applications where available;
- architecture-neutral runtimes;
- user-mode CPU translation for selected foreign-architecture applications;
- VM fallback where translation cannot satisfy correctness/performance.

Architecture translation is isolated behind the compatibility domain and must not leak into the shell contract.

## 8. Unified installation flow

Target UX:

1. user opens one application center or package interface;
2. application metadata identifies the domain/runtime requirements;
3. `aw-packaged` resolves trusted sources and runtime dependencies;
4. runtime/container/prefix is provisioned automatically;
5. application appears in the unified launcher;
6. accessibility capability is reported before first launch;
7. uninstall removes app state according to an explicit keep/delete-data choice.

## 9. Compatibility database

Maintain machine-readable test results per application/version/runtime revision.

Fields should include:

- launch
- install
- rendering
- keyboard
- mouse/touch
- audio
- clipboard
- files
- networking
- notifications
- accessibility tree
- focus
- text/caret/selection
- screen-reader usability
- known blockers

This database becomes a release gate and prevents regressions hidden behind a simple “application launches” result.

## 10. Current upstream baseline — 2026-09-11

Research baseline, not permanent pins:

- Wine stable 11.0; current development release observed: 11.17.
- Waydroid 1.6.3 includes initial Android 16 image support.
- Darling remains under active development; many GUI applications still require missing framework functionality.
- QEMU current stable family observed: 11.1.1.
- ReactOS remains useful as a compatibility/reference codebase but is not selected as the host kernel.

All production pins must be stored as exact revisions in reproducible build metadata rather than copied from this document.
