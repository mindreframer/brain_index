# EPIC004 Plan: Metadata Filters

## Progress

- [ ] Phase 4.1: Bitset module and unit tests.
- [ ] Phase 4.2: Filter compilation and both strategies.
- [ ] Phase 4.3: Elixir options and tests.
- [ ] Phase 4.4: QA; commit.

## Quality Gate

- [ ] Oracle filtered R@5: e5 ≥ 0.86, v5 ≥ 0.80.
- [ ] Filtered HNSW equals filtered exact on a forced-HNSW test (threshold 0), recall ≥ 0.95.
- [ ] Unknown keys or values give empty results, not errors.
- [ ] `bin/qa_check.sh` passes.

## Commit Rule

Commit as `roadmap001 - epic {k} - {title.lower()}` after all checks pass.
