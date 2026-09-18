# Native storage line

The native storage system starts from zero. NTFS, ReFS, ext4, XFS, Btrfs, ZFS, APFS and F2FS are research references, not implementation foundations.

## Design direction

The native primitive is not required to be a traditional file. The first design target is a persistent, versioned object graph from which file/directory views can later be projected for human use and compatibility.

The initial durable model uses:
- logical 4 KiB blocks;
- stable object identities;
- monotonically increasing generations;
- redundant durable anchors;
- explicit previous-known-good roots;
- transaction descriptors;
- a future authenticated integrity graph.

## Version-1 publication model

The intended order is:

1. Write candidate objects only to storage that is not part of the current committed root.
2. Issue a durability barrier.
3. Persist a `Prepared` transaction record.
4. Issue a durability barrier.
5. Derive a new anchor whose `previous_root` is the current committed root.
6. Publish that anchor into the inactive redundant-anchor slot.
7. Issue a durability barrier.
8. Only then may recovery select the new generation.

A prepared transaction by itself never replaces the active generation. Recovery selects only a valid durable anchor. If publication is missing or the new anchor is torn/corrupt, the previous valid anchor remains authoritative.

The Rust model now tests the state-transition rules above, but physical write ordering, flush/FUA/barrier semantics and cryptographic verification are **NOT IMPLEMENTED**. Therefore these tests are evidence for the publication state machine only, not proof of real-device crash consistency.

## Required properties before a filesystem milestone can be called complete

1. Crash consistency proven by fault injection across every physical write boundary.
2. Data and metadata integrity verification.
3. Redundant anchor recovery.
4. Explicit rollback to a known-good generation without accepting malicious rollback.
5. Atomic publication of a new root.
6. Native encryption and key-rotation design.
7. Versioning/snapshot semantics.
8. Repair and recovery paths usable without vision.
9. No dependency on a foreign filesystem implementation.
10. Reproducible benchmarks against relevant existing systems.

## Current implementation status

`aw-storage` currently defines and tests:
- versioned on-disk structural contracts;
- logical block addressing with overflow rejection;
- redundant-anchor selection after structural validation;
- transaction-generation monotonicity;
- derivation of a next-generation anchor from a prepared transaction;
- preservation of the previous known-good root;
- rejection of stale and non-prepared transactions;
- recovery staying on the old generation until a valid new anchor exists.

Cryptographic integrity verification, physical block I/O, durability barriers, allocator, object graph, real crash-fault injection, encryption, repair, snapshots and filesystem projections are **NOT IMPLEMENTED** and must not be reported as PASS.
