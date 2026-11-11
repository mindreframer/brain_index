# EPIC002 Spec: Exact Search

## Purpose

Correct reference search: metrics, vector arena and brute-force top-k. Every later optimisation is tested against it.

## Scope

- `Metric::{Cosine, Lorentz}` per ADR003; cosine normalisation; Lorentz validation (x0 >= 1).
- Vector arena (contiguous f32), external u64 id <-> internal u32 maps.
- Exact top-k with a bounded heap; ties broken by internal id.
- Temporary NIF: `new/2`, `insert/4`, `exact_search/3` for oracle tests.

## Acceptance Criteria

- Rust unit tests for the metrics against hand-computed values.
- Elixir oracle test: e5 brute force R@5 = 0.77 plain; v5 Lorentz R@5 = 0.67 plain (within 0.01).
- Dimension and invalid-vector errors are returned, not raised from Rust.
