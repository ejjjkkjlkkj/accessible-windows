# OS foundation

The `os` branch starts from an orphan history and is intentionally independent from earlier Accessible Windows implementation branches.

## Core rule

The native system is one organism. Accessibility, cognition, memory, security and interaction are not external frameworks attached to a visual operating system.

The system carries verified human-interaction state through every privileged transition. Visual output is one modality among others, not the source of truth. `aw-core` now defines the first native semantic object/graph contract: roles, states, actions and graph relations exist independently of any visual widget or external accessibility API.

## Bring-up order

1. Versioned boot ABI and deterministic diagnostics.
2. Native human-I/O contract carried by firmware and boot.
3. Keyboard path plus at least one verified non-visual output path.
4. UEFI entry and boot handoff.
5. x86-64 kernel entry, memory map and page tables.
6. Interrupts, timer and scheduler.
7. Capability/security foundation.
8. ACPI and PCI discovery.
9. Native audio, USB HID and braille transports.
10. Native semantic object/graph representation shared by all modalities. **FOUNDATION IMPLEMENTED; renderers not implemented.**
11. From-zero VFS and persistent storage foundation.
12. Process model, native shell and recovery environment.
13. Cognitive and memory primitives only after their invariants can be specified and tested.

## Human-I/O invariant

Capabilities are never inferred merely because hardware or a firmware protocol exists. They become available only after a native adapter verifies them.

The foundational transition requires:
- semantic interaction state;
- keyboard input;
- at least one verified non-visual output channel: speech or braille.

Serial output is an engineering diagnostic path and does not satisfy the non-visual human-output requirement.

## Current milestone

This milestone defines contracts and compile-time structure. The native semantic object/edge invariants are implemented and unit-tested, but speech, braille rendering, visual projection, haptics, agent projection, memory management, scheduling, drivers, cognition, persistent memory and the complete native filesystem are not implemented.
