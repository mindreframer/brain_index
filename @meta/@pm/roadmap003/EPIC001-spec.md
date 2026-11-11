# EPIC001 Spec: LMDB Engine

## Scope

- heed env + 7 tables per ADR009; config validation on reopen.
- HNSW insert/search over read/write transactions; tombstones; upsert.
- Postings rebuilt from `meta` on open; adaptive filter strategy kept.
- Unaligned SIMD distance on mapped bytes.
- Temp env for `new`, removed on drop; backup via env copy.

## Acceptance Criteria

- Rust tests: recall vs exact, filters, delete/upsert, reopen persistence, config mismatch.
