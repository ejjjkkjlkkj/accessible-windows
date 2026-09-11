# Accessibility Architecture

## Principle

Accessibility is part of the operating-system contract. A graphical feature that cannot be discovered, understood, operated, and recovered from without sight is incomplete.

## 1. Accessibility service

`aw-accessibilityd` owns the normalized semantic graph for the active user session.

It receives adapters from:

- native Wayland/Linux applications through AT-SPI and toolkit-specific bridges;
- Windows applications through UI Automation, MSAA and IAccessible2 bridges;
- Android applications through the Android accessibility framework;
- macOS-compatible applications through the compatibility runtime where semantic data is available;
- the Accessible Windows shell through a native first-party protocol.

The broker must preserve the originating platform semantics while exposing a stable common model.

## 2. Built-in screen reader

The built-in screen reader should be a first-party Rust service/client pair rather than a shell script around an external reader.

Core modules:

- focus tracking
- object navigation
- browse/document mode
- text/caret/selection tracking
- speech formatting
- braille formatting
- keyboard command layer
- event coalescing/deduplication
- application profiles
- virtual buffers for complex documents/web content
- secure-desktop mode

The project can compare behavior against NVDA and other established readers, but must remain independently implemented.

## 3. Boot and recovery accessibility

Accessibility must not begin only after the normal desktop starts.

Required phases:

### Boot manager

- predictable keyboard choices;
- optional simple tones for state/failure indication;
- no timed destructive action that cannot be interrupted from the keyboard.

### Initramfs / recovery

- USB audio and common integrated audio initialized as early as practical;
- local TTS available without network access;
- keyboard-only menus;
- storage/network recovery operations exposed as structured choices rather than raw visual logs only;
- optional serial console for engineering diagnostics.

### Installer

- screen reader starts from a documented keyboard command and can be configured to auto-start;
- language/voice/audio-device selection is keyboard accessible;
- partitioning has explicit semantic descriptions and confirmation;
- destructive actions announce target disk, size, model, filesystem and consequences before commit.

## 4. Semantic object requirements

Every interactive object must expose:

- role
- accessible name
- state
- value when applicable
- keyboard focusability
- available actions
- relationships
- position in collections when relevant
- shortcut/access key when present

Text controls additionally expose:

- full text or permitted text range
- insertion caret
- selection ranges
- line/word/character boundaries
- formatting attributes when semantically relevant
- editable/read-only state

## 5. Focus rules

- Only one logical keyboard focus per session.
- Focus changes generate ordered events.
- Modal surfaces trap focus intentionally and expose a semantic reason.
- Closing a modal restores focus to a deterministic valid object.
- Background compatibility domains may not steal focus silently.
- Focus must remain queryable even when the visual compositor is degraded.

## 6. Event pipeline

Events receive:

- monotonic timestamp
- source domain
- process/application identity
- object/runtime ID
- event type
- urgency
- optional text delta
- privacy classification

The pipeline performs:

- duplicate removal
- coalescing of noisy updates
- priority handling
- stale-event rejection
- back-pressure
- per-application throttling

No optimization may remove meaningful caret, focus, selection, error, or security events.

## 7. Passwords and secrets

Sensitive fields are classified at the broker boundary.

Rules:

- password contents are never logged;
- screen reader speaks configurable masked feedback only;
- braille output follows explicit secure-field policy;
- clipboard/history integration is disabled for protected content by default;
- compatibility adapters must not downgrade a protected control to ordinary text.

## 8. Speech

`aw-speechd` exposes a stable API independent of the selected synthesizer.

Minimum capabilities:

- speech queue priorities
- interrupt/cancel
- rate/pitch/volume
- language/voice switching
- spelling/character mode
- punctuation levels
- SSML or equivalent structured speech subset
- low-latency local synthesis

A local offline synthesizer is mandatory for installation and recovery. Network/cloud voices can be optional additions only.

## 9. Braille

`aw-brailled` handles:

- device discovery
- USB/Bluetooth transports
- translation tables
- routing keys
- status cells
- braille keyboard input
- contracted/uncontracted modes
- per-language tables

Braille must work at login and in recovery for supported hardware once the necessary driver stack is available.

## 10. Keyboard contract

Critical OS actions require keyboard equivalents.

Initial command families should include:

- start/stop/toggle screen reader
- speech interrupt
- next/previous focusable item
- object navigation
- read current item/window/status
- character/word/line navigation
- heading/link/form navigation for document mode
- task/application switching
- notifications
- quick settings
- recovery invocation

Shortcuts must be remappable and must avoid relying solely on touch gestures.

## 11. Compatibility-domain bridges

### Windows bridge

Map UIA/MSAA/IAccessible2 into the host graph. Preserve UIA text ranges, caret, selection, live regions, notifications and control patterns where available.

### Linux bridge

Map AT-SPI object/event data into the host graph. Native first-party shell components should bypass lossy translation and publish directly.

### Android bridge

Map AccessibilityNodeInfo hierarchy, actions, text, selection, content descriptions, collection semantics and accessibility events.

### macOS-compatible bridge

Map semantics exposed by the compatibility runtime. Do not synthesize misleading accessibility metadata for controls that cannot actually be operated.

## 12. Testing gates

Automated checks:

- unlabeled focusable controls
- duplicate IDs
- broken parent/child relations
- missing focus events
- focus loss after dialog close
- inaccessible error messages
- keyboard traps
- secret leakage
- excessive event storms
- missing text/caret interfaces

Real-user simulation:

- machine starts with monitor logically unavailable;
- installation by keyboard + speech only;
- login by keyboard + speech only;
- network connection setup;
- application installation and launch;
- file operations;
- update and rollback;
- recovery after failed boot;
- shutdown/restart.

A release fails if a critical path requires vision.
