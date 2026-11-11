# EPIC004 Spec: Metadata Filters

## Purpose

Exact metadata filtering per ADR004 that never loses recall on selective filters.

## Scope

- Per-(key, value) dense bitsets; filter = AND across keys of OR within a key.
- Adaptive strategy: brute force when allowed ≤ threshold (default 4096), else filtered HNSW.
- `search(index, q, k, filter: map, brute_force_threshold: n, ef: n)`.

## Acceptance Criteria

- Oracle filtered R@5: e5 ≥ 0.86, v5 ≥ 0.80.
- Filtered HNSW equals filtered exact on a forced-HNSW test (threshold 0), recall ≥ 0.95.
- Unknown keys or values give empty results, not errors.
