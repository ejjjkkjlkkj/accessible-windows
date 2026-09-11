# Accessible Windows

Accessible Windows is an experimental, clean-room operating-system project for modern PCs.

The target is a real x86-64 UEFI operating system that can eventually boot from USB, install to NVMe/SATA storage, start directly on physical hardware, provide a modern desktop, and treat accessibility as a core system contract rather than an optional application.

## Project principles

- Accessibility-first: every native interactive control must expose semantic information.
- Physical hardware first-class: virtual machines are test targets, not the final product.
- Memory safety where practical: Rust is the default implementation language for new privileged code.
- Clean-room compatibility: do not copy leaked or otherwise unauthorized proprietary Windows source code.
- Measurable quality: performance, boot time, memory use, accessibility and compatibility will be benchmarked.
- Recoverability: updates and system services are designed with rollback and recovery in mind.

## Current status

Bootstrap phase. The repository currently contains:

- a first x86-64 UEFI executable under `boot/uefi`;
- a `no_std` kernel/boot contract crate;
- a `no_std` accessibility semantic model crate;
- architecture and accessibility specifications;
- a staged roadmap toward an installable UEFI system;
- cross-platform CI plus a dedicated UEFI build job.

The UEFI job produces an `aw-uefi-boot.efi` build artifact. A complete bootable USB/disk image is the next milestone.

## Initial target

- Architecture: x86-64
- Firmware: UEFI
- Boot media: USB / EFI System Partition
- Installation target: NVMe/SATA SSD
- UI: native compositor and accessible UI toolkit
- Compatibility goals: native API first, then progressively Win32, Linux and web workloads

## Repository layout

```text
boot/
  uefi/                 First UEFI executable
crates/
  aw-kernel-contract/   Boot and kernel-facing data contracts
  aw-accessibility/     Semantic accessibility primitives and validation
docs/
  ARCHITECTURE.md
  ACCESSIBILITY.md
  ROADMAP.md
  LEGAL.md
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

Expected output:

```text
target/x86_64-unknown-uefi/release/aw-uefi-boot.efi
```

## Source policy

This project must remain clean-room. Public documentation, published specifications and appropriately licensed open-source projects may be used according to their licenses. Leaked or unauthorized proprietary operating-system source code must not be copied, translated or imported into this repository.

## License

A project license has not yet been selected. Until one is added, do not assume permission to redistribute repository code outside the rights provided by applicable law.
