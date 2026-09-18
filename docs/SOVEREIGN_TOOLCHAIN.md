# Sovereign Toolchain Specification

Status: **FOUNDATIONAL SPECIFICATION — IMPLEMENTATION NOT COMPLETE**

The project will own its language and complete development toolchain. Existing languages and toolchains may be used temporarily only as independent executable specifications or validators while the sovereign chain is being created. They are not the final base.

## Required project-owned tools

The sovereign line requires at least:

1. **Seed-0** — smallest auditable executable seed derived directly from CPU/firmware specifications.
2. **Assembler** — project syntax, encoder, relocation model and diagnostics.
3. **Linker / executable emitter** — project object model and deterministic image construction.
4. **Compiler** — project language parser, semantic analyzer, type/effect/capability checker and code generator.
5. **Build orchestrator** — dependency graph, deterministic scheduling, provenance and reproducible build manifests.
6. **Verifier** — structural invariants, authority reachability, accessibility invariants and proof artifacts.
7. **Debugger / diagnostics** — semantic state inspection without requiring vision.
8. **Image builder** — deterministic boot/recovery image construction.
9. **Package/update builder** — versioned, anti-rollback-aware and provenance-bearing updates.
10. **Test engine** — unit, property, model and integration tests.
11. **Adversarial engine** — fuzzing, malformed-state injection, capability abuse attempts, regeneration poisoning attempts and fault injection.
12. **Accessibility verifier** — proves that interactive paths have semantic identity, keyboard reachability and a non-visual projection.

## Not a basic compiler

The language/toolchain is not considered adequate if it only parses syntax and emits machine code.

The compiler architecture must eventually understand native concepts including:
- authority and capability flow;
- effect sets;
- memory ownership/lifetime;
- cell identity and generation;
- regenerative-state boundaries;
- provenance/trust class;
- semantic human interaction;
- resource budgets;
- concurrency/isolation boundaries;
- security epoch and anti-rollback constraints.

Unsafe escape paths, if any remain necessary at hardware boundaries, must be tiny, explicit, separately auditable and impossible to invoke from ordinary code without a dedicated authority.

## Accessibility contract for every tool

A project-owned tool is incomplete if a blind operator cannot perform its core workflow without relying on a visual-only representation.

Every interactive tool must expose one semantic source of truth consumed by:
- keyboard/navigation;
- speech;
- braille;
- visual rendering;
- automation/test harnesses.

Required properties:
- stable semantic identifiers for commands, errors, progress and results;
- complete keyboard operation;
- deterministic focus/navigation order where interaction is sequential;
- structured diagnostics carrying severity, source location, invariant and remediation context;
- no information conveyed only by color, position, animation or iconography;
- progress/event streams readable without polling a visual screen;
- machine-readable output generated from the same semantic records as human output.

## Engineering quality gate

"Works" is not enough.

Each project-owned tool must eventually prove:
- deterministic input/output contracts where applicable;
- bounded resource behavior for privileged phases;
- explicit error taxonomy;
- crash-safe writes for persistent outputs;
- no silent fallback that weakens security or accessibility;
- reproducible artifacts;
- stable versioned formats;
- adversarial handling of malformed input;
- recovery behavior for interrupted work;
- performance measurements with declared workload and hardware assumptions.

## Evidence A + B

No foundational property is labelled PASS from one test family alone.

### Evidence A — structural/static

Examples:
- grammar/specification conformance;
- type/effect/capability checks;
- authority reachability result;
- semantic-accessibility completeness check;
- deterministic build graph validation;
- object/image-format invariants;
- formal/model proof where feasible.

### Evidence B — independent execution

Examples:
- execute produced artifact;
- self-host compiler generation N+1 from N;
- compare independent rebuild outputs;
- boot generated image;
- fuzz malformed input;
- inject crash/fault boundaries;
- run hostile capability/regeneration cases;
- perform keyboard-only and non-visual workflow validation.

A property is PASS only when its required A and B evidence agree.

## "Better" requires comparison evidence

The project does not claim a tool is better merely because it is new.

A superiority claim must state:
- exact property being compared;
- benchmark or proof method;
- competing baseline/version;
- workload/hardware assumptions;
- raw evidence;
- regression threshold.

If this evidence is absent, wording must remain "design target" or "unproven".

## Bootstrap quality rule

Seed-0 may be deliberately tiny, but **tiny does not mean low quality**.

Every stage that becomes human-interactive must inherit the semantic/accessibility contract. Every stage that gains authority must inherit capability/provenance rules. Every stage that produces persistent artifacts must inherit reproducibility and crash-safety requirements.

The bootstrap chain is allowed to grow in capability, never to weaken the constitution.
