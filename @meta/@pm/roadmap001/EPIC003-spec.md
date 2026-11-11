# EPIC003 Spec: HNSW Graph

## Purpose

Approximate nearest-neighbour search with HNSW (Malkov and Yashunin), porting HyperspaceDB's insert/search structure.

## Scope

- Deterministic level assignment (seeded xorshift), M / M0 = 2M, ef_construction.
- Greedy descent on the upper layers; beam search on layer 0 with a visited-epoch scratch buffer.
- Neighbour-selection heuristic and bidirectional link pruning.
- `search/4` uses HNSW with `ef` (default max(64, k)).

## Acceptance Criteria

- Recall@10 versus exact ≥ 0.98 on the e5 and v5 fixtures.
- Oracle R@5 unchanged within 0.01.
- Deterministic: the same inserts give the same graph.
