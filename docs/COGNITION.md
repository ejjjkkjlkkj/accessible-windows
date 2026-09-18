# Native cognition foundation

Cognition is part of the OS core, not an external agent framework.

## Planning is not authority

A `CognitiveIntentV1` contains:
- a semantic goal;
- a concrete semantic target;
- memory/evidence provenance;
- requested rights;
- the system generation;
- the planner domain.

The intent can request a mutating operation, but the intent itself grants zero authority.

## Execution gate

Only `authorize_intent` may derive an `ExecutionPermitV1`, and only when:
- the intent validates;
- the capability validates;
- the capability subject is `AgentExecutor`;
- target matches exactly;
- requested rights are fully covered;
- the capability is active for the intent generation.

This keeps `AgentPlanner`, `AgentExecutor` and security authority separate.

## Current status

Implemented and unit-tested:
- evidence-bearing intent structure;
- planner-only intent production contract;
- exact capability match for agent execution;
- rejection of insufficient rights;
- rejection of non-executor capability subjects;
- generation-bounded execution permit derivation.

**NOT IMPLEMENTED:** perception engines, attention scheduling, reasoning algorithms, learning, goal selection, model inference, actuator/syscall execution, cryptographic evidence verification or hardware capability enforcement.
