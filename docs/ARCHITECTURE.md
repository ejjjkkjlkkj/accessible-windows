# OS foundation

The `os` branch starts from an orphan root commit. No source file or Git parent is inherited from an earlier Accessible Windows branch.

## Bring-up order

1. UEFI entry and deterministic diagnostics.
2. Keyboard-only firmware interaction.
3. Earliest non-visual output path.
4. Versioned boot handoff.
5. x86-64 kernel entry, memory map and page tables.
6. Interrupts, timer and scheduler.
7. ACPI and PCI discovery.
8. Audio, USB HID and braille transports.
9. Native semantic accessibility service.
10. User-space process model and accessible shell.

## Accessibility invariant

Accessibility capabilities are never inferred from the presence of hardware or firmware protocols. A capability becomes `available` only after a runtime adapter has verified it. Every transition can state capabilities it requires and the kernel can reject a transition whose mandatory accessibility contract is incomplete.

Serial output is an engineering fallback. It is not treated as speech or braille.

## Current milestone

This commit establishes only the contracts and compilation targets. It does not claim that speech, braille, keyboard firmware input, memory management, scheduling or drivers are implemented yet.
