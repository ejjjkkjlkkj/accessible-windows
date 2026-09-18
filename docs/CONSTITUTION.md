# System constitution

This document defines architectural constraints that code may not weaken.

## 1. One native system

The project builds one autonomous operating system from zero. Windows, Linux, Android and macOS may be compatibility targets or research references, never structural runtime dependencies.

## 2. Accessibility is intrinsic

Accessibility is not a subsystem. There is no accessibility service sitting above a visual interface and no external accessibility API is required for the native system.

Human interaction semantics are part of the system's primary state. Visual rendering, speech, braille, keyboard, haptics and automation are projections or consumers of the same native meaning.

A component or project-owned tool that cannot expose a valid non-visual interaction path is incomplete.

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

Every PASS must be backed by reproducible evidence.

For foundational tools and security properties, PASS requires both:
- **Evidence A — structural proof:** specification conformance, static invariants, authority/accessibility checks or a stronger formal result;
- **Evidence B — independent execution proof:** runtime tests, adversarial tests, self-hosting/reproducibility checks, accessibility validation, fault injection or equivalent independent evidence.

If either A or B is missing, the property is **UNPROVEN**, not PASS.

## 6. GitHub Actions is authoritative evidence, not the sovereign compiler base

GitHub Actions may execute, compare, fuzz and validate project artifacts. It must not define the final compiler, language runtime, assembler, linker or build-system architecture.

Temporary external tools may remain as independent validators while the sovereign chain is created. They are not part of the final bootstrap closure.

## 7. From-zero storage

The native VFS and persistent storage architecture will be designed from zero. Existing filesystems are research references only.

Crash consistency, integrity, provenance, versioning, recovery, encryption and accessibility of repair paths are first-class requirements.

## 8. Firmware to application continuity

The same architectural invariants continue through firmware, boot, recovery, kernel, drivers, storage, tools, UI and applications.

No stage is exempt because it runs "before the OS" or "only during development".

## 9. Sovereign language and toolchain

The final OS toolchain is project-owned. It includes:
- the language specification;
- compiler;
- assembler;
- linker/executable emitter;
- build orchestrator;
- verifier/proof tooling;
- debugger and diagnostics;
- package/update tooling;
- image/boot-media builder;
- test, fuzz and adversarial tooling;
- accessibility validation tooling;
- runtime and standard facilities.

The final sovereign build must not require Rust, Python, C, C++, GCC, Clang, LLVM or another third-party compiler/runtime as its implementation foundation.

The first executable seed is project-authored directly from public CPU/firmware interface specifications and is kept minimal enough to audit independently.

## 10. Tool quality is OS quality

Project-owned development tools are not exempt from OS requirements.

Every interactive tool must be:
- keyboard-complete;
- semantic-first rather than screen-layout-first;
- usable through non-visual output;
- deterministic where determinism is required for reproducibility;
- explicit about authority and side effects;
- machine-readable without hiding information from human-readable output;
- versioned and provenance-bearing;
- failure-transparent: errors identify the violated invariant and never silently downgrade safety.

A GUI may exist later, but no core capability may be GUI-only.

## 11. Clean-room implementation

No third-party source code is copied into fundamental native components or the sovereign toolchain.

External systems may be researched for documented properties, failures, proofs and benchmarks. Project code is written from our own specifications and tests. Research references do not become implementation ancestry.

## 12. Bootstrap closure

Sovereignty is not complete until:
1. the project-owned seed can create the first project-owned compiler/tool path;
2. project-owned tools build the next generation of project-owned tools;
3. the compiler can build itself;
4. repeated self-hosting converges to reproducible artifacts;
5. the OS can be rebuilt using only project-owned executable tooling plus documented hardware/firmware interfaces;
6. accessibility and security invariants remain valid throughout that chain.

Until these gates pass with Evidence A + B, the sovereign toolchain is a target under construction, not a completed claim.
