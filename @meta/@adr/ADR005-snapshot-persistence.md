# ADR005: Snapshot Persistence, Application as System of Record

- Status: Accepted
- Date: 2026-11-11
- Decision owners: BrainIndex maintainers

## Context

The index is derived data: every vector comes from a document plus an
embedding model that the application stores. HyperspaceDB's write-ahead log,
payload store and tiering are about 2,500 lines. In the spike, a crash left a
stale `security.db` lock that blocked restarts. Rebuilding 543 chunks took
150 ms in-process, so rebuilding 1 million vectors is expected to take
minutes, not hours.

## Decision

- Persistence is an explicit **snapshot**: `save(index, path)` writes the
  complete index, and `load(path)` restores it. No WAL, no background
  flushing, no lock files.
- File format: our own versioned binary format, little-endian. It contains a
  magic string `BRIX`, a format version, configuration, vectors, graph
  levels and links, external ids, tombstones, and metadata postings, with a
  trailing checksum (FNV-1a 64).
- Writes are atomic: write to `path.tmp`, fsync, then rename.
- Loading validates the magic, version, sizes and checksum. A corrupt file
  returns `{:error, %BrainIndex.Error{reason: :corrupt_snapshot}}`, and the
  code never panics.
- After a crash, the application reloads the last snapshot and re-inserts
  anything newer from its system of record, or rebuilds from scratch.

## Consequences

- Simple, crash-safe (atomic rename), and with nothing to recover.
- Writes between snapshots are lost on a crash. That's acceptable because the
  application can replay them.
- A snapshot holds the full index in one file. Incremental snapshots are
  unnecessary at current scale.

## Alternatives Considered

- **WAL with replay.** Rejected: complexity and failure modes we measured in
  the spike, for durability the system of record already provides.
- **Serde/bincode/rkyv.** Rejected to avoid dependencies. The format has five
  sections and is easy to write by hand and to test round-trips.
- **mmap-backed storage.** Deferred until indexes no longer fit comfortably
  in RAM.
