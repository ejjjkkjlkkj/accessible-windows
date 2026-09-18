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
7. Capability/security foundation. **STRUCTURAL FOUNDATION IMPLEMENTED:** `KernelAuthority`, `SystemSovereign`, planner/executor separation, explicit resource/right/lifetime scopes. Hardware-unforgeable enforcement and cryptographic delegation are not implemented.
8. ACPI and PCI discovery.
9. Native audio, USB HID and braille transports.
10. Native semantic object/graph representation shared by all modalities. **FOUNDATION IMPLEMENTED; renderers not implemented.**
11. From-zero VFS and persistent storage foundation.
12. Process model, native shell and recovery environment.
13. Native cognitive-memory primitives with explicit class, version, provenance, retention and confidence. **MEMORY CONTRACT FOUNDATION IMPLEMENTED; cognition/learning not implemented.**

## Authority invariant

`KernelAuthority` is an internal non-human authority and cannot be minted or delegated by `SystemSovereign` or an agent. `SystemSovereign` is the highest human system authority but is not kernel identity.

The cognitive planner may inspect information through explicit read capabilities but cannot receive mutating rights. An agent executor may act only through an explicit capability scoped to a semantic resource, exact rights and a generation window.

This is currently a structural software contract. Hardware-backed/unforgeable capabilities, cryptographic grant chains and revocation storage are not implemented.

## Human-I/O invariant

Capabilities are never inferred merely because hardware or a firmware protocol exists. They become available only after a native adapter verifies them.

The foundational transition requires:
- semantic interaction state;
- keyboard input;
- at least one verified non-visual output channel: speech or braille.

Serial output is an engineering diagnostic path and does not satisfy the non-visual human-output requirement.

## Current milestone

This milestone defines contracts and compile-time structure. The native semantic object/edge invariants and the structural cognitive-memory record contract are implemented and unit-tested. Learning, retrieval, cognition/planning, persistent-memory storage integration, speech, braille rendering, visual projection, haptics, agent projection, memory management, scheduling, drivers and the complete native filesystem are not implemented.
