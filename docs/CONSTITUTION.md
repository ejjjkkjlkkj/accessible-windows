# Sovereign system constitution

## 1. Zero software ancestry

The sovereign OS is designed and implemented from zero.

No existing operating system, kernel, language runtime, compiler framework, filesystem, accessibility framework or UI toolkit is an implementation base.

External projects may be researched to understand public properties and failures. Their source code is not copied into the sovereign line and their architecture is not inherited as a mandatory skeleton.

## 2. Hardware boundary only

The project is software. It does not manufacture CPUs, GPUs, memory controllers or firmware silicon.

Documented instruction sets and hardware/firmware interfaces may be implemented because real hardware requires them. Those interfaces are boundaries, not the OS architecture.

A firmware adapter is replaceable boundary code. The sovereign kernel, language, object model, security, accessibility and tools must not depend structurally on one firmware family.

## 3. Project-owned toolchain

The final chain is entirely project-owned:
- Seed-0;
- instruction encoder/assembler;
- linker/image emitter;
- AWL compiler;
- verifier/prover;
- build orchestrator;
- debugger/diagnostics;
- package/update tooling;
- test/fuzz/adversarial tooling;
- accessibility proof tooling.

External CI hosts may temporarily witness proofs. They are never part of the sovereign bootstrap closure.

## 4. Accessibility is a universal invariant

Accessibility is not a service, API, plugin, compatibility layer or separate subsystem.

Every sovereign artifact has an accessibility obligation and an A+B proof record.

Interactive components must provide one semantic source of truth, complete keyboard operation, and a verified non-visual path. Visual output may be a projection but can never be the sole source of meaning or control.

Non-interactive components are not exempt. They must prove that they do not introduce human-visible-only state and that errors/status/provenance can be carried into the native semantic system without loss.

No early-boot, recovery, developer, diagnostic, security or maintenance path is exempt.

## 5. A + B or UNPROVEN

For every foundational property:

**Evidence A** = structural proof from specification, types, invariants, static reachability, byte map, or stronger formal evidence.

**Evidence B** = independent proof through execution, reconstruction, adversarial testing, fault injection, cross-environment comparison, or an equivalent independent witness.

A property is PASS only when both required evidence classes pass.

## 6. No ambient authority

Nothing receives authority merely because it executes.

Authority must be explicit, scoped, provenance-bearing, generation-bound and non-self-expanding.

Cognition may propose intent but cannot mint its own authority, rewrite the security floor or self-authorize privileged effects.

## 7. Regenerative architecture

The native architecture separates:
- genome: what can be built;
- morphology: what should exist and where;
- memory: lived/learned mutable state.

A compromised cell is never the source of truth for its own reconstruction. Regeneration uses verified genome/morphology and transfers only explicitly validated state. Old capabilities do not survive regeneration.

## 8. Human meaning precedes presentation

Semantic identity and actions exist before visual, speech, braille or haptic projection.

A component that cannot preserve this invariant is incomplete.

## 9. Claim discipline

"Better", "secure", "accessible", "sovereign", "self-hosted", "bootable" and similar claims require their declared A+B evidence.

Missing evidence is written as `UNPROVEN`; it is not hidden behind optimistic wording.
