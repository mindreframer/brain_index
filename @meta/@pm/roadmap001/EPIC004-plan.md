# EPIC004 Plan: Metadata Filters

## Progress

- [x] Phase 4.1: Bitset module and unit tests.
- [x] Phase 4.2: Filter compilation and both strategies.
- [x] Phase 4.3: Elixir options and tests.
- [x] Phase 4.4: QA; commit.

## Quality Gate

- [x] Oracle filtered R@5: e5 ≥ 0.86, v5 ≥ 0.80.
- [x] Filtered HNSW equals filtered exact on a forced-HNSW test (threshold 0), recall ≥ 0.95.
- [x] Unknown keys or values give empty results, not errors.
- [x] `bin/qa_check.sh` passes.

## Commit Rule

Commit as `roadmap001 - epic {k} - {title.lower()}` after all checks pass.
