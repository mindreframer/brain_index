# EPIC003 Plan: HNSW Graph

## Progress

- [x] Phase 3.1: Graph structure and level RNG.
- [x] Phase 3.2: Insert with layer search and selection heuristic.
- [x] Phase 3.3: Search API and recall tests.
- [x] Phase 3.4: QA; commit.

## Quality Gate

- [x] Recall@10 versus exact ≥ 0.98 on the e5 and v5 fixtures.
- [x] Oracle R@5 unchanged within 0.01.
- [x] Deterministic: the same inserts give the same graph.
- [x] `bin/qa_check.sh` passes.

## Commit Rule

Commit as `roadmap001 - epic {k} - {title.lower()}` after all checks pass.
