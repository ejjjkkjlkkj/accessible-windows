# Sovereign Accessible OS

The `os` branch is the clean-room sovereign operating-system line.

## Absolute rule

The active sovereign tree is not based on Windows, Linux, Android, macOS, Rust, Python, C, C++, GCC, Clang, LLVM, an existing kernel, an existing runtime, an existing accessibility framework, or an existing application framework.

Public hardware specifications may be used only as boundary contracts required to execute on real hardware. They do not define the internal architecture.

Everything fundamental is project-owned:
- language;
- compiler;
- assembler/encoder;
- linker/image emitter;
- object format;
- build graph;
- verifier;
- debugger;
- package/update format;
- kernel;
- storage;
- security;
- cognition and memory;
- regenerative cells;
- human semantics and accessibility;
- applications and tools.

## Accessibility is mandatory everywhere

Every sovereign artifact has an accessibility proof record.

- **A** proves the structural invariant.
- **B** proves it independently by execution, reconstruction, adversarial testing, or another independent witness.

No component is exempt because it is "internal", "early boot", "developer-only" or "not a GUI".

A missing A or B is `UNPROVEN`. It is never silently treated as PASS.

See `docs/CONSTITUTION.md`, `docs/ACCESSIBILITY.md`, `docs/PROOF_MODEL.md`, and `docs/ORIGIN.md`.
