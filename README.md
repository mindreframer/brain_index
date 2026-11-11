# BrainIndex

An embedded vector index with exact metadata filters for Elixir, written in Rust and loaded as a
NIF. It is the retrieval core of the company brain. Internal package, not published.

```elixir
{:ok, index} = BrainIndex.new(dim: 768, metric: :cosine)

:ok =
  BrainIndex.insert_many(index, [
    {1, embedding_1, %{"kind" => "kb", "lang" => "de"}},
    {2, embedding_2, %{"kind" => "outreach", "performance" => "good"}}
  ])

{:ok, hits} = BrainIndex.search(index, query, 5, filter: %{"kind" => ["kb", "ticket"]})
# => [{1, 0.1234}, ...]   {id, distance}, nearest first

{:ok, true} = BrainIndex.delete(index, 2)
:ok = BrainIndex.save(index, "brain.brix")
{:ok, index} = BrainIndex.load("brain.brix")
BrainIndex.info(index)
# => %{dim: 768, metric: :cosine, size: 1, deleted: 1, m: 16, ef_construction: 200}
```

## What it does, and what it doesn't

| BrainIndex owns | Your application owns |
|---|---|
| vectors, HNSW search, exact metadata filters | embeddings (e.g. Bumblebee + multilingual-e5) |
| upsert / delete | documents, chunk text, payloads |
| atomic snapshots (`save` / `load`) | the system of record, replays after a crash |
| concurrency-safe access from any process | retrieval policy per agent, multi-node distribution |

- **Metrics:** `:cosine` (normalised; distance `1 - cos`) and `:lorentz` (hyperbolic, time component
  first).
- **Filters:** exact match. `%{"kind" => "kb", "lang" => ["de", "en"]}` means
  `kind = kb AND lang IN (de, en)`. Filters that allow at most 4096 vectors run as exact brute
  force, so they never lose recall. Use low-cardinality keys (kind, lang, performance, tenant),
  not unique ids.
- **Concurrency:** searches run in parallel. Writes take an exclusive lock, so batch them with
  `insert_many`. Heavy calls run on dirty schedulers.
- **Errors:** always `{:error, %BrainIndex.Error{reason: atom}}`. A native panic becomes
  `:internal` and never crashes the VM.
- **Persistence:** explicit snapshots only. There's no WAL. The index is derived data, so rebuild
  it or reload it from your system of record.

## Quality

The tests use real spike data as an oracle: 543 multilingual chunks and 150 labelled queries,
embedded with `multilingual-e5-base` and YARlabs `v5` (Lorentz).

| check | result |
|---|---|
| e5 recall@5, plain / `kind` filter | 0.76 / ≥ 0.86 (HNSW equals exact search) |
| v5-Lorentz recall@5, plain / `kind` filter | 0.673 / ≥ 0.80 |
| HNSW top-10 overlap with exact search | ≥ 0.98 |
| full test suite | about 1 s (Elixir) + about 0.5 s (Rust) |

Performance (10k random 384-dimension vectors, Apple Silicon): HNSW p50 274 µs, filtered brute
force p50 64 µs, build 3.5k vectors/s. See `bench/RESULTS.md`.

## Development

```sh
bin/qa_check.sh                         # format, warnings-as-errors, ExUnit, rustfmt, clippy, cargo test
MIX_ENV=prod mix run bench/bench.exs    # benchmark
```

Requires Elixir ≥ 1.18 and Rust 1.91 (pinned in `rust-toolchain.toml`). The only dependency is
`rustler`, in both Elixir and Rust.

Design decisions are in `@meta/@adr` (ADR001–ADR006). The roadmap is in `@meta/@pm`. Read
`AGENTS.md` before changing anything.
