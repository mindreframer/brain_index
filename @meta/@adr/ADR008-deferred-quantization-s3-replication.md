# ADR008: Deferred — Quantization, S3 Shipping, Replication

- Status: Accepted (deferral with design constraints)
- Date: 2026-11-11
- Decision owners: BrainIndex maintainers

## Context

Three capabilities are expected to matter later: quantization (less RAM), S3 upload and download
(backup, fast node bootstrap), and replication (local-first sync between nodes, possibly with
CRDT semantics). None is needed for the first company-brain workloads, which have thousands to
low millions of vectors on one node. Building them now adds code and test surface without a
measured need. The risk to avoid is having to bolt them on awkwardly later.

## Decision

Don't implement them now. Keep the current design compatible with each one:

### Quantization

- **When:** more than about 2–5 million 768-dimension vectors per node, or RAM pressure. At 768D,
  f32 costs 3 KiB per vector, so 1 million vectors is about 3 GiB.
- **How:** an `int8` scalar codebook per index (4× smaller), or ternary (16× smaller; the spike
  showed no recall loss). HNSW traverses the quantized vectors and the top `ef` candidates are
  reranked with the full f32 vectors, kept in RAM or mmap.
- **Compatibility:** it is a new index option and a new snapshot format version (`BRIX` v2). The
  log carries full-precision vectors, so existing logs remain valid. The public API stays the
  same.

### S3 upload and download

- **Units:** checkpoint files and *closed* log segments (ADR007) are immutable. They can be
  uploaded as they are and verified by name and CRC.
- **How:** a separate `Storage` backend or a shipper process that uploads closed segments and
  checkpoints, and bootstraps a new node by downloading the newest checkpoint plus later
  segments. This can probably reuse the `objectstorex` package.
- **Compatibility:** no format change is needed.

### Replication (local-first)

- **Substrate:** the LSN-ordered operation log. Ops are idempotent per id (upsert replaces,
  delete tombstones), which gives last-writer-wins register semantics per id when ops carry a
  (timestamp, node) version. That is a simple state-based CRDT.
- **How:** nodes exchange log segments or op ranges. A per-id version vector or hybrid logical
  clock resolves conflicts.
- **Compatibility:** an `op` may need an origin and version field later. Log records are
  self-describing Erlang terms, so a new field can be added without breaking replay of old
  records.

## Consequences

- ROADMAP002 delivers only the WAL and a demo application.
- Each deferred capability has a stated trigger and a path that requires no API break.
