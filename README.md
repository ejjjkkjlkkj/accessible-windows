# Accessible Windows

Accessible Windows is an experimental, clean-room operating-system project for modern x64 PCs.

The target is a real x86-64 UEFI operating system that can eventually boot from USB, install to NVMe/SATA storage, start directly on physical hardware, provide a modern desktop, and treat accessibility as a core system contract rather than an optional application.

## Architecture scope

Accessible Windows is **x86-64/x64 only**. ARM and ARM64 are out of scope for the project, its kernel, boot path, CI matrices and release artifacts. This keeps engineering effort focused on one real PC architecture and allows deeper hardware, driver, compatibility and accessibility validation.

## Project principles

- Accessibility-first: every native interactive control must expose semantic information.
- Physical hardware first-class: virtual machines are test targets, not the final product.
- x64-only: all supported boot, kernel, driver and release paths target x86-64.
- Memory safety where practical: Rust is the default implementation language for new privileged code.
- Clean-room compatibility: do not copy leaked or otherwise unauthorized proprietary Windows source code.
- Measurable quality: performance, boot time, memory use, accessibility and compatibility will be benchmarked.
- Recoverability: updates and system services are designed with rollback and recovery in mind.

## Current status

Bootstrap phase. The repository currently contains:

- an x86-64 UEFI executable under `boot/uefi`;
- a separate x86-64 freestanding kernel image;
- UEFI memory-map and GOP/display discovery;
- a GPT disk-image builder with a FAT32 EFI System Partition;
- an OVMF/QEMU smoke test that executes the real disk image and enters the native kernel after `ExitBootServices`;
- a `no_std` kernel/boot contract crate;
- a `no_std` accessibility semantic model crate;
- architecture and accessibility specifications;
- x64-only Rust CI on Linux, Windows and Intel macOS.

The generated disk image is a boot prototype, not an operating-system installer yet.

## Initial target

- Architecture: x86-64 / x64 only
- Firmware: UEFI
- Boot media: USB / EFI System Partition
- Installation target: NVMe/SATA SSD
- UI: native compositor and accessible UI toolkit
- Compatibility goals: native API first, then progressively Win32, Linux and web workloads

## Repository layout

```text
boot/
  uefi/                 First x86-64 UEFI executable
kernel/
  x86_64/               Freestanding native x86-64 kernel
crates/
  aw-kernel-contract/   Boot and kernel-facing data contracts
  aw-kernel-core/       Kernel handoff validation
  aw-acpi/              ACPI validation primitives
  aw-accessibility/     Semantic accessibility primitives and validation
docs/
  ARCHITECTURE.md
  ACCESSIBILITY.md
  ROADMAP.md
  LEGAL.md
  REAL-HARDWARE-TEST.md
scripts/
  build-uefi-disk.sh    Builds the GPT/FAT32 UEFI disk image
```

## Build workspace

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Build UEFI executable

```bash
rustup target add x86_64-unknown-uefi
cargo build --manifest-path boot/uefi/Cargo.toml --target x86_64-unknown-uefi --release
```

Expected EFI output:

```text
boot/uefi/target/x86_64-unknown-uefi/release/aw-uefi-boot.efi
```

On Linux with `gdisk`, `dosfstools` and loop-device support, build the GPT/ESP image with:

```bash
./scripts/build-uefi-disk.sh
```

Expected disk image:

```text
build/accessible-windows-uefi-x86_64.img
```

The image is intended for controlled boot testing. It is not yet an installer and must not be written over a disk containing data you need.

## Source policy

This project must remain clean-room. Public documentation, published specifications and appropriately licensed open-source projects may be used according to their licenses. Leaked or unauthorized proprietary operating-system source code must not be copied, translated or imported into this repository.

## License

A project license has not yet been selected. Until one is added, do not assume permission to redistribute repository code outside the rights provided by applicable law.
