# Accessible Windows v0.1.0 Microsoft Preview

Date: 2026-09-24

This preview packages the smallest coherent UEFI accessibility implementation suitable for external technical review.

## Included

- semantic UEFI screen-reader core;
- live HII/IFR adapter;
- stable control identity and transactional snapshot refresh;
- bounded navigation and item chooser;
- password-value redaction;
- prioritized/preemptive speech scheduling;
- golden HII/HDA reference path and V2 integration target;
- hosted compile, sanitizer, static-analysis and OVMF validation;
- deterministic USB image builder and SHA-256 evidence.

## Repository cleanup

The release branch removes the active clutter of historical labs, superseded experiments and more than one hundred legacy workflow definitions from the published tree. Those materials remain in repository history/archival branches rather than the release candidate.

## Evidence boundary

Hosted CI proves the software gates for the exact release commit. Previously recorded Windows/VMware/PsExec evidence is documented separately and is not promoted to a claim about untested future commits.

## Not claimed

- complete Windows or UEFI screen-reader parity;
- OEM-independent physical audio support;
- production firmware safety certification;
- Windows Recovery Environment integration;
- final speech-quality acceptance.
