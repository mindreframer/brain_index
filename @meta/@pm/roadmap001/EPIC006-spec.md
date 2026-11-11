# EPIC006 Spec: Performance

## Purpose

Make the hot paths fast without changing results.

## Scope

- NEON (aarch64) and AVX2+FMA (x86_64) dot-product kernels with runtime detection and a scalar fallback.
- `insert_many/2` with vector binaries (f32 LE) as the fast path.
- Dirty CPU scheduling for search, insert_many, save and load.
- Benchmark script (`bench/bench.exs`) reporting build time and p50/p99 search.

## Acceptance Criteria

- SIMD and scalar kernels agree within 1e-5.
- Results identical to before (oracle tests unchanged).
- Documented benchmark numbers on fixtures.
