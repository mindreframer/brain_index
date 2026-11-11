# EPIC003 Plan: HNSW Graph

## Progress

- [ ] Phase 3.1: Graph structure and level RNG.
- [ ] Phase 3.2: Insert with layer search and selection heuristic.
- [ ] Phase 3.3: Search API and recall tests.
- [ ] Phase 3.4: QA; commit.

## Quality Gate

- [ ] Recall@10 versus exact ≥ 0.98 on the e5 and v5 fixtures.
- [ ] Oracle R@5 unchanged within 0.01.
- [ ] Deterministic: the same inserts give the same graph.
- [ ] `bin/qa_check.sh` passes.

## Commit Rule

Commit as `roadmap001 - epic {k} - {title.lower()}` after all checks pass.
