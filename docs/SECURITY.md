# Native authority and capability foundation

Security authority is explicit system state, not ambient administrator/root power.

## Authority domains

- `KernelAuthority`: internal, non-human kernel authority.
- `SystemSovereign`: highest human system authority; never an alias for kernel identity.
- `FirmwareAuthority`: pre-boot authority.
- `RecoveryAuthority`: recovery authority.
- `AgentPlanner`: cognition/planning domain. It cannot receive mutating capability rights.
- `AgentExecutor`: action domain. It requires explicit scoped capabilities.
- `UserProcess`: ordinary process domain.

## Capability rule

A capability identifies:
- a concrete semantic resource;
- exact rights;
- a start generation;
- an expiry generation;
- issuer, subject and optional parent grant.

There is no "all-powerful agent" right and no arbitrary kernel-memory-write right in the current capability vocabulary.

## Current evidence and limits

Implemented and unit-tested:
- planner/executor separation;
- distinct KernelAuthority/SystemSovereign identities;
- prevention of human/agent minting of KernelAuthority grants;
- resource-scoped rights;
- generation-bounded grants;
- explicit authorization checks.

**NOT IMPLEMENTED:** cryptographic signatures, unforgeable hardware capabilities, persisted revocation, delegation-chain verification, temporal clock leases, IOMMU/device capabilities, process enforcement and kernel syscall enforcement.
