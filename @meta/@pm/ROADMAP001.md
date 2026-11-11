# ROADMAP001 — Embedded Vector Index for the Company Brain

- **Status:** Completed at `0.1.0` (2026-11-11)
- **Target release:** `0.1.0` (internal, not published)
- **Primary interface:** Elixir
- **Native implementation:** Rust via Rustler (only dependency: `rustler`)

## 1. Product Goal

BrainIndex gives Elixir applications a fast, embedded vector index with exact
metadata filters. It is the retrieval core of the company brain.

```elixir
{:ok, index} = BrainIndex.new(dim: 768, metric: :cosine)

:ok =
  BrainIndex.insert_many(index, [
    {1, vec1, %{"kind" => "kb", "lang" => "de"}},
    {2, vec2, %{"kind" => "ticket", "lang" => "en"}}
  ])

{:ok, hits} = BrainIndex.search(index, query_vec, 5, filter: %{"kind" => "kb"})
# [{1, 0.123}, ...]  (id, distance), ascending distance

:ok = BrainIndex.save(index, "brain.brix")
{:ok, index} = BrainIndex.load("brain.brix")
```

## 2. Public Model

- `BrainIndex`: create, insert, insert_many, delete, search, size, info,
  save, load.
- `BrainIndex.Error`: a stable error struct.
- Ids are application-chosen unsigned 64-bit integers. Inserting an existing
  id replaces it (upsert).
- Metadata is a map of string keys to string values, filterable by exact
  match.

## 3. Out of Scope

Embedding models, chunking, payload or text storage, keyword search,
replication, a network server, quantization, and numeric range filters.
See the ADRs for the reasons.

## 4. Quality Bar

- Oracle acceptance: with the spike fixtures, e5 R@5 ≥ 0.86 filtered and
  ≥ 0.75 plain (exact: 0.76). v5 Lorentz R@5 ≥ 0.80 filtered and ≥ 0.66
  plain (exact: 0.673). With default `ef`, HNSW must equal exact recall.
- HNSW top-10 recall versus brute force ≥ 0.98 on fixtures.
- The full test suite (Elixir + Rust) runs in under 10 s after compilation.
- No warnings: `mix compile --warnings-as-errors`, `cargo clippy -D warnings`,
  and formatting checks.

## 5. Epics (dependency order)

| Epic | Outcome |
|---|---|
| EPIC001 | Package skeleton, NIF loads, QA gate, fixtures |
| EPIC002 | Exact search: metrics, vector arena, brute force, oracle parity |
| EPIC003 | HNSW graph: insert and search, recall ≥ 0.98 versus brute force |
| EPIC004 | Metadata filters: bitsets, adaptive brute force / filtered HNSW |
| EPIC005 | Mutations and persistence: upsert, delete, snapshot save/load |
| EPIC006 | Performance: SIMD kernels, insert_many, benchmarks |
| EPIC007 | Elixir API hardening, docs, end-to-end oracle acceptance |

## 6. ADRs

ADR001 (embedded index), ADR002 (clean port), ADR003 (metrics), ADR004
(filters), ADR005 (persistence), ADR006 (NIF boundary).
