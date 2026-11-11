# Benchmark results

`MIX_ENV=prod mix run bench/bench.exs 10000 384`. Random Gaussian vectors (the worst case for HNSW,
because there's no cluster structure), Apple Silicon (aarch64, NEON), 2026-11-11.

| metric | value |
|---|---|
| build (insert_many, m=16, ef_construction=200) | 2.87 s (3,480 vectors/s) |
| HNSW search k=10 | p50 274 µs, p99 591 µs |
| filtered HNSW, 10% selectivity (forced) | p50 972 µs, p99 1,944 µs |
| brute-force filter, 10% selectivity (default path) | p50 64 µs, p99 200 µs |
| exact search k=10 | p50 852 µs, p99 1,788 µs |
| snapshot | 16 MiB, save 38 ms, load 24 ms |

Takeaways:

- Brute force over a selective filter is about 15× faster than filtered HNSW, which supports the
  default `brute_force_threshold` of 4096 (ADR004).
- Fixture-sized indexes (543 × 768) search in well under 1 ms.
