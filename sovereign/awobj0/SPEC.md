# AWOBJ0 — sovereign object format generation 0

Status: **SPECIFICATION V0 — EMITTER/LOADER NOT YET PROVEN**

AWOBJ0 is the project-owned intermediate object representation for the sovereign toolchain.

## Canonical byte order

All multi-byte integers are little-endian in generation 0.

## Header

The first eight bytes are the ASCII bytes:

`41 57 4F 42 4A 30 30 30`

which spell `AWOBJ000`.

The fixed header then contains:
- format version: u32, value 0;
- header size: u32;
- target ISA id: u32;
- section count: u32;
- semantic manifest offset: u64;
- provenance manifest offset: u64;
- flags: u64, reserved and required to be zero.

## Required sections

Generation 0 defines:
- CODE — executable machine bytes;
- DATA — immutable data;
- CELL — cell/authority/effect descriptors;
- SEMANTIC — human/system semantic identities;
- PROVENANCE — source/specification identity and generation;
- PROOF — Evidence-A records emitted by the compiler.

No section grants authority merely by existing. Authority is evaluated when the object is admitted into a concrete system generation.

## Determinism

For identical normalized source, compiler generation, target and declared inputs, section ordering and serialized bytes must be deterministic.

## Safety

Unknown required section kinds are rejection conditions. Reserved fields must be zero. Integer ranges and offsets must be validated before dereference.

## Evidence

A conforming emitter/reader remains UNPROVEN until:
A. structural invariants are checked independently;
B. round-trip/execution/adversarial tests agree with those invariants.
