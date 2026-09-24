# Repository status

Updated: 2026-09-24

## Consolidation state

- Repository: `ejjjkkjlkkj/accessible-windows`
- Clean source line: `uefi-screenreader-live-integration-v2-20260922`
- Source head: `238c9dd4b22662c1b92314548a784a934504c45d`
- Consolidation branch: `repo-clean-consolidation-20260924`
- Earlier core line: `uefi-screenreader-core-v2-20260922`
- Relationship: live integration is 18 commits ahead of the core-v2 line.
- Legacy `main` is an unrelated orchestration history and is intentionally not force-rewritten during this cleanup pass.

## Verified physical baseline

GitHub Actions run `35846636674` completed successfully on 2026-09-23 using the Windows self-hosted runner and PsExec LocalSystem execution.

Project commit used by that successful run:

`8302a95b0b4d2fe4d57e2832bcc945819728b80d`

Verified markers from that run:

- `PSEXEC_INTERACTIVE_SYSTEM=PASS`
- `PSEXEC_REQUIRED=PASS`
- `IDENTITY_SID=S-1-5-18`
- `REAL_WINDOWS_VOICE_BUILD=PASS`
- `VOICE_CODEC_ROUNDTRIP=PASS`
- `NAVIGATION_EFI_REAL_VOICE_BUILD=PASS`
- `VMWARE_UEFI_BOOT=PASS`
- `VMWARE_SCREENREADER_DISCOVERY=PASS`
- artifact upload completed successfully
- `SYSTEM_SPEECH_OUTPUT_RATE=16000`

## Regression isolated

The current legacy-main workflow was later changed to pin project commit:

`6eded9e26b11b29f9643e1bb8eb694798eafa1fd`

Run `35848464855` failed in the physical PsExec stage immediately after:

`STAGE=SYSTEM_VOICE_READY`

The workflow also began requiring a 24 kHz marker and additional voice-bank markers that were not part of the last verified green baseline. Those requirements are therefore treated as targets, not as already-proven facts.

## Open closure gate

Issue #4, **UEFI screen reader core parity closure**, remains open. In particular, final parity still requires real HII control semantics, state/value speech, safe editing/activation, form hierarchy navigation, punctuation/value preservation, speech cancellation, larger prompt capacity, VM proofs and human-confirmed physical audible speech.

No final parity PASS should be asserted before those conditions are closed.

## Cleanup rules

1. Do not delete historical evidence branches until their unique proof is integrated or archived.
2. New consolidated work should be based on this clean source line rather than unrelated orphan histories.
3. Pin external project revisions in hardware workflows.
4. Keep baseline proof gates separate from new quality targets such as 24 kHz speech.
5. Never convert an experimental target into PASS without matching evidence.
