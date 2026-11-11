# ADR002: Clean Purpose-Built Crate, Porting HyperspaceDB Algorithms

- Status: Accepted
- Date: 2026-11-11
- Decision owners: BrainIndex maintainers

## Context

We need HNSW search, cosine and Lorentz distances, and metadata filtering.
HyperspaceDB (MIT) implements all of these, but inside about 21k lines across
`hyperspace-core`, `hyperspace-index` and `hyperspace-store`. The inventory
(`../EXTRACTION_PLAN.md`) found:

- `hyperspace-index/src/lib.rs` is one 4,556-line file. It references
  `QuantizationMode::` 67 times across 11 modes, reads 10 environment
  variables, and mixes MRL, wave search, BM25 and graph optimisation into the
  core HNSW paths.
- We would use about 10–15% of the code.
- It stores vectors as `f64`: 6,152 bytes per 768-dimension vector.
- It is a fast-moving project with one main vendor.

## Decision

Write a **small, purpose-built Rust crate** inside this Elixir package
(`native/brain_index_nif`), porting the *algorithms* rather than copying files:

- Port the HNSW insert and search procedure (greedy descent, layer-0 beam
  search, neighbour-selection heuristic, link pruning), cosine and Lorentz
  distances, and the idea of a brute-force fallback for selective filters.
- Store vectors as `f32`. Configure through explicit function parameters,
  never environment variables.
- The only Rust dependency is `rustler`. Bitsets, the random-number generator,
  the snapshot format and SIMD kernels are small local modules.
- Keep the YARlabs MIT notice in `THIRD_PARTY_NOTICES.md`, and mark ported
  routines in comments.

The spike vectors and recall numbers serve as the test oracle: e5 R@5
0.77 plain / 0.87 filtered, and v5-Lorentz 0.67 / 0.81 with exact search.

## Consequences

### Positive

- About 2k lines we fully understand, instead of about 21k we don't.
- No upstream churn. We change it only when we need to.
- Half the RAM per vector.
- No transitive dependency tree to audit.

### Negative

- We own correctness. That's mitigated by oracle tests and brute-force
  equivalence tests.
- Upstream performance work (quantization, io_uring, NUMA) doesn't reach us
  automatically. We can port it selectively if benchmarks call for it.

## Alternatives Considered

### Fork and trim HyperspaceDB crates

Rejected. Untangling the 4.5k-line `lib.rs` from 11 quantization modes and
environment-driven config costs more than writing the needed parts, and leaves
code shaped for problems we don't have.

### Depend on an HNSW crate (`hnsw_rs`, `usearch`, `instant-distance`)

Rejected for now. These pull C++ or large dependency trees (`usearch`), lack a
Lorentz metric, or don't expose filtered search with a brute-force fallback.
This choice can be revisited without an API change, because the Elixir API
does not leak the implementation.
