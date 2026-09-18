# Clean-room origin record

The sovereign line is authored from project-owned specifications.

## Allowed references

- public CPU instruction-set specifications;
- public hardware bus/device specifications needed for hardware support;
- public firmware interface specifications needed at machine boundaries;
- academic/public descriptions of properties, failures and proof techniques.

## Forbidden implementation ancestry

Fundamental sovereign source must not be copied from:
- existing kernels or operating systems;
- existing compiler/runtime source;
- existing filesystem implementations;
- existing accessibility frameworks;
- existing UI frameworks;
- third-party bootloader source.

The project may implement a documented external hardware interface in original code when required to run on real hardware.

## Historical code

Earlier Rust/Cargo experiments on this branch were research prototypes. They are removed from the active sovereign tree and are not part of the sovereign bootstrap ancestry.

Git history retains them as historical evidence; the active tree defines the current architecture.

## Proof

Origin is supported by chronological Git history, project-owned specifications, canonical hashes, independent witnesses during bootstrap, and explicit `UNPROVEN` status where implementation does not yet exist.
