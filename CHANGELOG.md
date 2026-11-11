# Changelog

## 0.2.0 — 2026-11-11

- `BrainIndex.Collection`: durable, supervised index with a write-ahead log (ADR007).
- `BrainIndex.Storage` behaviour with `Storage.File` (segments, CRC, torn-tail repair, checkpoints)
  and `Storage.Memory` (tests).
- `BrainIndex.to_binary/1` and `from_binary/1`; snapshots fsync their directory.

## 0.1.0 — 2026-11-11

First internal release (ROADMAP001).

- Embedded HNSW vector index as a Rust NIF; `rustler` is the only dependency.
- Metrics: cosine and Lorentz (hyperbolic).
- Exact-match metadata filters with an adaptive brute-force / filtered-HNSW strategy.
- Upsert, delete (tombstones), `insert_many` (all-or-nothing validation).
- Atomic `BRIX` v1 snapshots with checksums.
- NEON and AVX2+FMA distance kernels.
- Tagged errors and panic containment at the NIF boundary.
