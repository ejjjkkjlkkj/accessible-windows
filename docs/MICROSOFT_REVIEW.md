# Microsoft technical review brief

## Purpose

Accessible Windows explores a native accessibility path before the Windows desktop is available. The immediate research target is a UEFI screen reader that can expose firmware configuration semantics through speech without depending on a running Windows accessibility stack.

## What is implemented in this preview

- semantic extraction from HII/IFR package data;
- stable question/control identity;
- bounded focus navigation and searchable item chooser;
- spoken label, value, role and state formatting;
- password-value redaction;
- transactional refresh so malformed firmware data does not replace the last known-good semantic tree;
- priority/preemption behavior for spoken events;
- freestanding x86-64 UEFI build;
- OVMF runtime smoke validation;
- HDA-oriented V2 firmware link;
- reproducible USB boot-image assembly with hashes.

## Evidence model

Software validation and physical evidence are intentionally separated.

The exact release commit is accepted only when the `release-validation` GitHub Actions workflow succeeds. A previously successful Windows self-hosted run (`35846636674`, 2026-09-23) is retained as a physical/VMware baseline, but it exercised a separately pinned payload and is not presented as proof for future commits.

## What is not claimed

This preview is not a complete replacement for Narrator, NVDA or a production OEM firmware accessibility implementation. It does not claim complete coverage of UEFI controls, final speech quality, OEM-independent HDA routing, recovery-environment integration or production certification.

## Review questions for Microsoft

1. Which UEFI/HII accessibility semantics would be most useful to standardize or expose consistently across OEM firmware?
2. Are there existing Windows boot/recovery accessibility interfaces that could consume or preserve semantic pre-OS state?
3. Which security boundaries should a firmware accessibility component satisfy before integration with Windows recovery or setup flows?
4. Which hardware/firmware conformance environments would be most appropriate for broader validation?

## Repository pointers

- `README.md` — project scope and release layout.
- `docs/STATUS.md` — exact evidence boundary and open gates.
- `docs/RELEASE_NOTES.md` — preview contents.
- `SECURITY.md` — low-level security boundary.
- `boot/uefi-screenreader-core-v1/` — canonical screen-reader core.
