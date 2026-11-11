# EPIC001 Plan: LMDB Engine

## Progress

- [x] Phase 1.1: heed env + 7 tables per ADR009; config validation on reopen.
- [x] Phase 1.2: HNSW insert/search over read/write transactions; tombstones; upsert.
- [x] Phase 1.3: Postings rebuilt from `meta` on open; adaptive filter strategy kept.
- [x] Phase 1.4: Unaligned SIMD distance on mapped bytes.
- [x] Phase 1.5: Temp env for `new`, removed on drop; backup via env copy.

## Quality Gate

- [x] Rust tests: recall vs exact, filters, delete/upsert, reopen persistence, config mismatch.
- [x] `bin/qa_check.sh` passes.

## Commit Rule

`roadmap003 - epic {k} - {t.lower()}`
