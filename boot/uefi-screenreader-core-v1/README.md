# UEFI Screen Reader Core v1

This directory is a clean-room rewrite of the UEFI screen-reader interaction core. It is intentionally isolated from the proven boot path.

## Non-negotiable invariants

- Do not modify the golden boot carrier while the new core is being validated.
- No dynamic allocation in the core.
- Every buffer is fixed-size and bounded.
- Password values are never emitted in spoken output.
- Separators are never focus targets.
- Disabled controls remain discoverable but must be blocked by the future action adapter.
- A new focus announcement must not accumulate behind stale focus speech.
- Dialogs and critical alerts can preempt lower-priority interruptible speech.
- Critical speech can be marked non-interruptible.
- Speech completion always selects the highest-priority pending event first.
- UEFI side effects are outside this core: this module decides navigation and speech, not firmware writes.

## Architecture

1. HII/IFR adapter builds a semantic snapshot of firmware controls.
2. Navigator owns the virtual focus, role navigation, first-letter navigation, paging and Item Chooser.
3. Formatter emits concise speech in this order: label, value, role, state, position, hint.
4. HII adapter converts bounded firmware records into a transactional semantic snapshot.
5. Focus formatter stays concise; hints are emitted separately at low priority so navigation cancels them immediately.
6. Speech scheduler applies duplicate suppression, coalescing, priorities and preemption.
7. Audio adapter performs the physical stop/start operation and reports completion.

This split is deliberate: firmware parsing, interaction policy and HDA playback are independently testable.

## Current behavior

- Up/down equivalent next/previous navigation.
- Home/End and page movement.
- First-letter jumps.
- Role rotor next/previous.
- Searchable Item Chooser with incremental filtering, next/previous, backspace, select and cancel.
- Semantic focus utterances with state and position.
- HII/IFR semantic snapshot adapter with bounded fixed storage and all-or-nothing failure.
- Low-priority hints separated from immediate focus speech.
- Strict password value redaction at both adapter and formatter layers.
- Priority speech queue with duplicate suppression and bounded capacity.
- Preemption for dialogs and critical alerts.

## Quality gates

The CI workflow compiles the core in freestanding mode, runs behavior tests under AddressSanitizer and UndefinedBehaviorSanitizer, runs Clang static analysis, and proves that the existing realtime boot path has no diff from the v7 baseline commit.

## Next integration gate

The next code step is an adapter that maps live HII/IFR objects to SrItem without changing BOOTX64.EFI or KERNEL.BIN, then routes existing keyboard events through this core. Only after QEMU runtime evidence is green should the old monolithic navigation policy be replaced.
