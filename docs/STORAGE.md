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

## Required properties before a filesystem milestone can be called complete

1. Crash consistency proven by fault injection across every write boundary.
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

`aw-storage` currently defines only:
- versioned on-disk structural contracts;
- logical block addressing with overflow rejection;
- redundant-anchor selection after structural validation;
- transaction-generation monotonicity.

Cryptographic integrity verification, physical block I/O, allocator, object graph, journaling/commit protocol, encryption, repair, snapshots and filesystem projections are **NOT IMPLEMENTED** and must not be reported as PASS.
