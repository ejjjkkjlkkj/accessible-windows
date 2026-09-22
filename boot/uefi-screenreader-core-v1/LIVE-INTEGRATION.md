# Live HII integration v2

This branch keeps the validated golden firmware path byte-for-byte untouched while moving the screen-reader interaction layer to the V2 core.

## Proven boundary

The live adapter accepts the same EFI_HII_PACKAGE_LIST byte stream returned by EFI_HII_DATABASE_PROTOCOL.ExportPackageLists.

Pipeline:

1. validate package-list length and every package header;
2. select Forms packages;
3. validate IFR opcode boundaries transactionally;
4. derive stable control identity from package GUID + FormId + QuestionId;
5. resolve Prompt and Help through a bounded string callback;
6. resolve non-password current values through a bounded value callback;
7. map firmware flags into semantic state;
8. publish SrHiiRecord only after each control is internally consistent;
9. feed the resulting snapshot to SrScreenReaderSession;
10. preserve the existing HDA/DMA backend until the semantic path is proven in QEMU and hardware.

## Safety properties

- The golden HII/HDA firmware source is not modified on this branch.
- No dynamic allocation is introduced.
- Malformed package or IFR lengths reject the new snapshot.
- A value resolver failure rejects the new snapshot instead of replacing a known-good session.
- Password value callbacks are never invoked.
- Read-only question state is surfaced to speech.
- Stable IDs survive prompt-token changes for the same firmware question.
- Duplicate semantic IDs reject the snapshot.
- A valid HII End package is mandatory and must terminate the declared package-list length.
