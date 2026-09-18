# Universal accessibility invariant

Accessibility applies to every sovereign artifact without omission.

## Interactive artifact

An artifact is interactive when it can directly request, receive or present human intent, state, choice, error, progress, warning, recovery action or configuration.

It must have:

### Evidence A
- semantic identity independent of presentation;
- semantic role/state/action model;
- complete keyboard operation for fundamental actions;
- at least one required native non-visual output path;
- no information conveyed only by position, color, animation, pixels, iconography or pointer gesture;
- structured error/progress/status records;
- no foreign accessibility framework dependency.

### Evidence B
- keyboard-only execution test;
- non-visual execution test;
- semantic/event trace comparison with the same operation;
- negative test proving a visual-only implementation is rejected;
- recovery/error-path test, not only the success path.

## Non-interactive artifact

Non-interactive does not mean exempt.

### Evidence A
- no human-facing state hidden in raw visual/pixel state;
- errors/status are structured data;
- provenance and semantic identifiers are preserved when state crosses into an interactive component;
- no inaccessible emergency/diagnostic bypass exists.

### Evidence B
- malformed/error paths produce structured machine records;
- an independent consumer can project those records without parsing a visual screen;
- fault tests prove no silent downgrade to visual-only output.

## Early boot and recovery

Early boot and recovery have the same requirements as normal operation.

A transition that would make the machine usable only visually must fail closed or remain in a non-destructive waiting state until an allowed accessible path exists.

## Development tools

Compiler, assembler, linker, verifier, debugger, build tool and package/update tools are part of the product architecture.

Their core functions must be operable non-visually and must expose the same semantic record stream to human and automation consumers.

## Registry rule

`sovereign/a11y0/registry.awa11y0` is the current canonical proof registry.

Every active sovereign artifact must have exactly one registry record containing:
- path;
- class: interactive or noninteractive;
- A status;
- B status;
- invariant identifier.

The repository policy fails if an active sovereign artifact is missing from the registry.

A status may be `PASS`, `UNPROVEN` or `FAIL`. Release gates accept only required `PASS + PASS`.


## Proof truthfulness

A byte hash, file-presence check or cross-host checkout is not by itself Evidence B for accessibility.

Evidence B for accessibility requires behavior: an independent non-visual consumer, keyboard path, error/recovery path, or equivalent executable witness appropriate to the artifact.

Until such a witness exists, B remains `UNPROVEN`.
