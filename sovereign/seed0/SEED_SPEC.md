# Seed-0 specification

Status: **DESIGN GATE — NO BOOTABLE SEED CLAIMED YET**

Seed-0 is the smallest project-authored executable root of the sovereign software toolchain.

## Non-negotiable properties

- Its executable bytes are authored from the documented CPU/firmware instruction and image-format specifications, not emitted by a third-party compiler or assembler.
- Every byte range has a project-owned explanation.
- It contains no third-party runtime or library.
- It is intentionally too small to hide a general-purpose toolchain.
- It performs only the minimum transition necessary to validate and load the next project-owned tool stage.
- It does not grant ambient authority.
- If it becomes interactive, it must satisfy the same semantic/non-visual accessibility contract as every other project-owned tool.

## Required proof before PASS

Evidence A:
- complete byte map;
- instruction-by-instruction intent;
- entry/exit ABI;
- memory ranges;
- authority/resource assumptions;
- image-format invariants.

Evidence B:
- independent byte reconstruction from the byte map;
- execution under at least two independent machine/firmware environments or one emulator plus real hardware;
- negative tests for corrupted header, ranges and next-stage identity;
- exact hash comparison of independently reconstructed images.

Until those gates pass, Seed-0 status is **UNPROVEN** and no bootability claim is permitted.
