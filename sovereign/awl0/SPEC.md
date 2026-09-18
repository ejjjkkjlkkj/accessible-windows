# AWL0 — sovereign language generation 0

Status: **SPECIFICATION V0 — COMPILER NOT YET PROVEN**

AWL0 is the first project-owned language specification for the sovereign OS line. Its semantics are defined here, not inherited from Rust, C, Python or another programming language.

## 1. Design invariants

AWL0 programs are built from **cells**. A cell has:
- stable semantic identity;
- explicit generation;
- private state;
- explicit authority;
- explicit effects;
- explicit regeneration policy;
- explicit human-interaction contract when it interacts with a person.

There is no ambient filesystem, network, process table, global device namespace or superuser authority in the language model.

## 2. Fundamental value classes

AWL0 defines these semantic classes:

- `plain` — ordinary immutable value.
- `own` — uniquely owned value; transfer invalidates the source binding.
- `share` — immutable shared value.
- `cap` — non-duplicable authority token.
- `keep` — state eligible for validated transfer into a later cell generation.
- `shed` — state destroyed with the current cell generation.
- `untrusted` — data that cannot drive privileged effects without an explicit validation transition.
- `trusted` — data accepted by a named validator and carrying that provenance.

These are language semantics, not library conventions.

## 3. Authority rule

A flow may perform only effects declared in its signature and backed by a capability binding.

Authority may be:
- narrowed;
- transferred;
- consumed;
- expired;
- revoked by a later security generation.

Authority may never be widened by ordinary program code.

## 4. Cell syntax

Generation-0 canonical shape:

```text
cell <Name> {
    identity <semantic-id>
    generation <generation-source>

    authority {
        require <capability>
        deny <capability>
    }

    memory {
        keep <name> : <type>
        shed <name> : <type>
    }

    human {
        semantic <semantic-contract>
        path <modality-list>
    }

    flow <name>(<inputs>) -> <output> {
        effects <effect-list>
        body <statements>
    }
}
```

The syntax is intentionally small in generation 0. New syntax cannot be added without versioning the grammar.

## 5. No implicit effects

The following operations are impossible without declared effects and matching authority:
- persistent write;
- network transmit;
- device command;
- authority delegation;
- cell differentiation;
- regeneration;
- security-generation change;
- executable-code publication.

Reading a textual name such as `"disk"` or an integer that happens to equal an address creates no authority.

## 6. Regeneration semantics

A regenerable cell has two disjoint state regions:
- `keep`: values that pass a named validator before transfer;
- `shed`: values that are destroyed.

Capabilities are never in `keep`. A new generation receives freshly evaluated authority.

Generation N cannot reuse a capability minted for generation N-1.

## 7. Human interaction semantics

Any cell that declares `human` must provide:
- semantic identity independent of visual layout;
- complete keyboard reachability for its fundamental actions;
- at least one valid non-visual projection for foundational workflows;
- structured status/error/progress events.

A visual projection may consume the same semantic state but cannot be the source of truth.

## 8. Failure semantics

AWL0 has no silent privilege fallback.

When an invariant fails, execution produces a structured failure record:
- invariant identifier;
- cell identity;
- generation;
- source span if applicable;
- requested effect;
- authority involved;
- remediation context when deterministic.

## 9. Concurrency baseline

Generation 0 forbids implicit mutable sharing between cells.

Inter-cell communication is message transfer through explicitly authorized endpoints. Capability transfer is false by default and must be declared separately from data transfer.

## 10. Proof obligations

A compiler claiming AWL0 conformance must produce Evidence A:
- parse/grammar conformance;
- ownership checks;
- authority/effect closure;
- regeneration-state separation;
- human-interaction completeness where declared.

It must also pass Evidence B:
- execute canonical positive specimens;
- reject canonical negative specimens;
- reproduce identical semantic/object output from identical inputs;
- survive malformed-input tests without expanding authority.

Until both pass, the compiler is **UNPROVEN**.


## 11. Mandatory accessibility proof

Every AWL source artifact has an A11Y0 registry record.

A human-interactive cell is invalid unless its structural evidence declares semantic identity, keyboard reachability and a required non-visual path.

Compilation of a future AWL implementation must reject interactive code whose A evidence is absent or fails. Release admission additionally requires independent B evidence.

There is no accessibility opt-out annotation.
