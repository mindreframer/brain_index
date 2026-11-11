# Benchmark results

`MIX_ENV=prod mix run bench/bench.exs 10000 384`. Durable LMDB index (fsync per commit), random
Gaussian vectors, Apple Silicon (16 KiB pages, NEON), 2026-11-11. Disk bytes are the process's
actual writes (`proc_pid_rusage`).

| metric | LMDB engine (0.3.0) | in-memory engine (0.1.0) |
|---|---|---|
| build, batches of 1000 | 751 vectors/s, **30 MiB written for 14.6 MiB raw (≈2×)** | 3,480 vectors/s (no durability) |
| single insert + commit | p50 8.9 ms, **≈520 KB written** | — |
| group commit (`BrainIndex.Writer`, 50 concurrent callers) | 200 inserts in 0.55 s, 4 commits, **≈43 KB written per insert (12× less)** | — |
| HNSW search k=10 | p50 1.1 ms, p99 1.9 ms | p50 0.27 ms |
| filtered (10%) brute force | p50 0.29 ms | p50 0.06 ms |
| exact k=10 | p50 2.6 ms | p50 0.85 ms |
| reopen (cold, incl. postings rebuild) | **2 ms** | load full snapshot |
| process RSS | 143 MiB | whole index in RAM |
| data file | 23.6 MiB for 10k × 384 | — |

## Interpretation

- **Batches are SSD-friendly:** about 2× write amplification including the graph, metadata and
  payloads. Nothing is ever rewritten wholesale.
- **Single-item commits are not.** A copy-on-write B-tree copies each touched page (16 KiB here),
  and an HNSW insert touches about 30 neighbour lists. That's about 0.5 MB per lone insert. Use
  `insert_many`, or the group-commit writer (`BrainIndex.Writer`), which merges concurrent writes
  into one transaction.
- **Searches cost about 1 ms** (B-tree lookups instead of array indexing). That's fine for agent
  retrieval. The hot path reads mapped pages with no copies.
