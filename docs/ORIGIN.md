# Origin and provenance ledger

Status: **ACTIVE CLEAN-ROOM RECORD**

This ledger records the origin rules for the sovereign OS/toolchain line.

## Purpose

The project is written from project-owned specifications. External systems may be studied for public facts, properties, failures, proofs and benchmarks, but their source code is not an implementation base for the sovereign line.

Git history proves the chronology and exact bytes committed to this repository. It does not by itself grant ownership over general ideas, algorithms, standards or independently created work.

## Clean-room rules

1. No third-party source file is copied into `sovereign/`.
2. No generated output from a third-party compiler is accepted as canonical sovereign source.
3. CPU and firmware standards may be consulted because they define hardware interfaces, not software ancestry.
4. Research notes must describe properties in our own words and must not be pasted into implementation files.
5. Every sovereign format starts with a project-owned specification before an implementation is declared conformant.
6. Every implementation claim requires Evidence A + B.
7. If ancestry is uncertain, the component is quarantined and not used in the sovereign chain.

## Evidence A — structural origin

CI verifies that:
- canonical sovereign source/specification files exist;
- forbidden dependency declarations are absent from sovereign source formats;
- the language and object format carry project-specific version/magic identifiers;
- each fundamental component has an origin declaration and status.

## Evidence B — independent byte identity

CI computes canonical SHA-256 digests on independent hosted operating systems. The hashes must agree for the same Git commit.

The Git commit/tree identifiers provide an additional content-addressed history independent of the SHA-256 evidence emitted by the jobs.

## Initial sovereign artifacts

- `sovereign/awl0/SPEC.md` — generation-0 language semantics.
- `sovereign/awl0/foundation.awl` — first canonical language specimen.
- `sovereign/awobj0/SPEC.md` — generation-0 object representation.
- `sovereign/seed0/SEED_SPEC.md` — rules for the hand-authored executable seed.
- `sovereign/ORIGIN.manifest` — machine-readable origin declaration.

## Claim discipline

The current artifacts prove an original, versioned specification line and reproducible byte identity.

They do **not** yet prove:
- a self-hosting compiler;
- a complete assembler/linker;
- a bootable Seed-0;
- semantic superiority over another language;
- complete toolchain sovereignty.

Those remain **UNPROVEN** until their own A+B gates pass.
