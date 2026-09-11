# Unsafe-code policy

Accessible Windows is Rust-first, but an operating system cannot eliminate every unsafe operation. Firmware transitions, CPU instructions, MMIO, interrupt setup and context switching require carefully bounded unsafe code.

## Rules

- Safe Rust is the default.
- Unsafe code must be isolated at a hardware, firmware or FFI boundary.
- Every unsafe block must have a `SAFETY:` comment describing the invariant relied upon.
- Higher-level policy, accessibility semantics and ordinary system services should remain safe Rust wherever practical.
- CI must exercise unsafe transition paths that can be tested deterministically.

## Current audited boundary

The initial UEFI stage contains one explicit unsafe transition: `uefi::boot::exit_boot_services`.

Before that call, the boot stage drops every scoped UEFI protocol object and temporary boot-services memory map. Only copied scalar handoff data remains. After the call, the smoke-test path does not invoke UEFI Boot Services again and reports progress through x86 debugcon.
