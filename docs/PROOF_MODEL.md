# A+B proof model

## Status values

- `PASS` — evidence exists and the referenced gate succeeded.
- `UNPROVEN` — implementation/evidence is incomplete.
- `FAIL` — evidence contradicts the required invariant.

No implicit PASS exists.

## Proof independence

A and B must not be the same check restated twice.

Examples:
- static authority proof + hostile runtime attempt is independent;
- byte specification + independently reconstructed bytes is independent;
- semantic contract + keyboard/speech/braille execution trace is independent.

## Accessibility proof identifier

The current foundation invariant is:

`A11Y0.FOUNDATION.NO-HUMAN-PATH-WITHOUT-SEMANTICS`

It means that no human-facing path may exist unless meaning and action are represented semantically and can be consumed without vision.

## CI as witness

GitHub Actions is currently an external witness only.

Its shell, operating system and hosted tools are not part of the sovereign toolchain and cannot satisfy the final self-hosting claim.

The final proof runner will itself be project-owned.
