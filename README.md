# Accessible Windows OS

The `os` branch is an independent, from-zero operating-system line.

## Non-negotiable line

- The OS must remain structurally independent from Windows, Linux, Android and macOS.
- Accessibility is not an API, service, framework or optional layer. It is an invariant of every system component and every project-owned interactive tool.
- Cognition, memory, security, storage, recovery and human interaction are designed as intrinsic properties of one system.
- Firmware, boot, kernel, filesystem, drivers, tools and user environment are developed as native components of this OS.
- The final language, compiler, assembler, linker, build system, verifier and development tools are project-owned.
- External toolchains may validate transitional work but are not the final bootstrap base.
- A feature or "better than" claim is not accepted without reproducible **Evidence A + B**.

See `docs/CONSTITUTION.md`, `docs/ARCHITECTURE.md` and `docs/SOVEREIGN_TOOLCHAIN.md`.
