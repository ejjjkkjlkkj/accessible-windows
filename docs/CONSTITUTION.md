# System constitution

This document defines architectural constraints that code may not weaken.

## 1. One native system

The project builds one autonomous operating system from zero. Windows, Linux, Android and macOS may be compatibility targets or research references, never structural runtime dependencies.

## 2. Accessibility is intrinsic

Accessibility is not a subsystem. There is no accessibility service sitting above a visual interface and no external accessibility API is required for the native system.

Human interaction semantics are part of the system's primary state. Visual rendering, speech, braille, keyboard, haptics and automation are projections or consumers of the same native meaning.

A component that cannot expose a valid non-visual interaction path is incomplete.

## 3. No external accessibility framework

Native code must not depend structurally on UI Automation, MSAA/IAccessible, AT-SPI, AXAPI, Android Accessibility, AccessKit, Electron, GTK or Qt to make the OS usable.

Compatibility layers may implement foreign interfaces at the boundary, but the native system cannot rely on them internally.

## 4. Cognition and memory are native architecture

Cognition and memory are not bolt-on applications. Their future implementations must use native system primitives, security domains, provenance and explicit capabilities.

The system architecture must support:
- working state;
- persistent memory;
- episodic history;
- semantic knowledge;
- procedural knowledge;
- perception;
- attention;
- reasoning;
- planning;
- self-evaluation;
- action.

No placeholder is allowed to claim these features before implementation and validation.

## 5. Capability and evidence first

Critical transitions must be explicit, versioned and reject invalid state. Capabilities are observed, never assumed.

Every PASS must be backed by a test, proof, measurement, boot trace or other reproducible evidence.

## 6. GitHub Actions is authoritative

Builds, tests, policy checks and release evidence are orchestrated by versioned GitHub Actions workflows.

Manual local success is useful for debugging but is not release evidence.

## 7. From-zero storage

The native VFS and persistent storage architecture will be designed from zero. Existing filesystems are research references only.

Crash consistency, integrity, provenance, versioning, recovery, encryption and accessibility of repair paths are first-class requirements.

## 8. Firmware to application continuity

The same architectural invariants continue through firmware, boot, recovery, kernel, drivers, storage, UI and applications.

No stage is exempt because it runs "before the OS".
