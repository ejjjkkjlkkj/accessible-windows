# A11Y0 proof-record format

Status: SPECIFICATION V0

A11Y0 is the project-owned accessibility evidence record format.

Each non-comment record is one line:

`path|class|A|B|invariant`

Fields:
- `path`: canonical repository-relative artifact path;
- `class`: `interactive` or `noninteractive`;
- `A`: `PASS`, `UNPROVEN`, or `FAIL`;
- `B`: `PASS`, `UNPROVEN`, or `FAIL`;
- `invariant`: project-owned accessibility invariant identifier.

No omitted field has a default.

A release proof runner must reject duplicate paths and missing active artifacts.
