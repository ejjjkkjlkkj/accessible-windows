# Accessible Windows

Accessible Windows is a low-level accessibility project targeting native UEFI and pre-OS screen-reader capabilities.

## Current repository state

The historical `main` branch is the hardware-orchestration line. The coherent source line has been consolidated in:

`repo-clean-consolidation-20260924`

Consolidation commit:

`b53b62895ddb57094aa944cb1cdc549f0523668d`

It is based on `uefi-screenreader-live-integration-v2-20260922`, which contains the live HII/IFR adapter and HDA integration work.

## Verified physical baseline

The last verified green physical PsExec/VMware run used project commit:

`8302a95b0b4d2fe4d57e2832bcc945819728b80d`

Verified baseline includes LocalSystem PsExec execution, native voice build, codec round-trip, NAVIGATION.EFI build, VMware UEFI boot and screen-reader discovery.

The verified speech baseline is 16 kHz. The 24 kHz voice-quality target is not yet marked PASS.

## Current gate

Issue #4 remains open for UEFI screen-reader parity closure. No final parity claim should be made until its HII semantics, editing, hierarchy navigation, speech cancellation, capacity, VM and physical-human validation requirements are closed.

## Development rule

Keep historical proof branches until their unique evidence is integrated or archived. New coherent source work should continue from `repo-clean-consolidation-20260924`.
