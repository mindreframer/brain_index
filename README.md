# BrainIndex

An embedded vector index with exact metadata filters for Elixir, written in Rust and loaded as a
NIF. It is the retrieval core of the company brain. Internal package, not published.

```elixir
{:ok, index} = BrainIndex.open("/var/lib/brain", dim: 768)     # LMDB on disk; reopen = instant recovery

:ok =
  BrainIndex.insert_many(index, [
    {1, embedding_1, %{"kind" => "kb", "lang" => "de"}, chunk_text_1},
    {2, embedding_2, %{"kind" => "outreach", "performance" => "good"}, chunk_text_2}
  ])                                                          # one transaction, durable on return

{:ok, hits} = BrainIndex.search(index, query, 5, filter: %{"kind" => ["kb", "ticket"]}, payload: true)
# => [{1, 0.1234, chunk_text_1}, ...]   {id, distance, payload}, nearest first

{:ok, true} = BrainIndex.delete(index, 2)
:ok = BrainIndex.backup(index, "/backups/brain-2026-11-11")     # consistent online copy
{:ok, _copied} = BrainIndex.compact(index, "/var/lib/brain.next") # drop tombstones (ADR011)
BrainIndex.info(index)
# => %{dim: 768, metric: :cosine, size: 1, deleted: 1, m: 16, ef_construction: 200}

{:ok, scratch} = BrainIndex.new(dim: 768)                       # temporary index for tests
```

## Storage: LMDB, on disk, incremental

- Vectors, HNSW links, metadata and payloads live in one LMDB environment (ADR009).
- Each write call is a single transaction. It writes **only the pages that changed** and then
  fsyncs. There are no snapshots, no WAL and no periodic full rewrites.
- **The data doesn't need to fit in RAM.** Searches read straight from the memory map, and the
  kernel keeps hot pages cached. In RAM: the page cache plus filter bitsets (1 bit per node per
  distinct metadata pair).
- Many concurrent single writes? Use `BrainIndex.Writer` (group commit). It merges them into one
  transaction, which means 12× fewer bytes written to the SSD than one commit per insert.
- After a crash the index is exactly the last committed state. Recovery takes no time and there
  is nothing to replay. A `SIGKILL` test checks that every acknowledged write survives.

## What it does, and what it doesn't

| BrainIndex owns | Your application owns |
|---|---|
| vectors, HNSW search, exact metadata filters, payloads | embeddings (e.g. Bumblebee + multilingual-e5) |
| upsert / delete, durable commits, backups | the original documents / system of record |
| concurrency-safe access from any process | retrieval policy per agent, multi-node distribution |

- **Metrics:** `:cosine` (normalised; distance `1 - cos`) and `:lorentz` (hyperbolic, time component
  first).
- **Filters:** exact match. `%{"kind" => "kb", "lang" => ["de", "en"]}` means
  `kind = kb AND lang IN (de, en)`. Filters that allow at most 4096 vectors run as exact brute
  force, so they never lose recall. Use low-cardinality keys (kind, lang, performance, tenant),
  not unique ids.
- **Concurrency:** searches run in parallel. Writes take an exclusive lock, so batch them with
  `insert_many` (one commit per batch). Heavy calls run on dirty schedulers.
- **Errors:** always `{:error, %BrainIndex.Error{reason: atom}}`. A native panic becomes
  `:internal` and never crashes the VM.
- **Persistence:** LMDB (ADR009). Quantization, S3 shipping and replication are deferred, with a
  design path in ADR008. A backup directory is the S3 unit.

## Quality

The tests use real spike data as an oracle: 543 multilingual chunks and 150 labelled queries,
embedded with `multilingual-e5-base` and YARlabs `v5` (Lorentz).

| check | result |
|---|---|
| e5 recall@5, plain / `kind` filter | 0.76 / ≥ 0.86 (HNSW equals exact search) |
| v5-Lorentz recall@5, plain / `kind` filter | 0.673 / ≥ 0.80 |
| HNSW top-10 overlap with exact search | ≥ 0.98 |
| full test suite | about 1 s (Elixir) + about 0.5 s (Rust) |

Performance (10k × 384, durable, Apple Silicon): HNSW p50 about 1.1 ms, batched build 751 vectors/s
with about 2× write amplification, reopen 2 ms. See `bench/RESULTS.md`.

## Development

```sh
bin/qa_check.sh                         # format, warnings-as-errors, ExUnit, rustfmt, clippy, cargo test
MIX_ENV=prod mix run bench/bench.exs    # benchmark
```

Requires Elixir ≥ 1.18 and Rust 1.91 (pinned in `rust-toolchain.toml`). The only dependency is
`rustler` (Elixir) and `rustler` + `heed`/LMDB (Rust, ADR009).

Design decisions are in `@meta/@adr` (ADR001–ADR010). The roadmap is in `@meta/@pm`. Read
`AGENTS.md` before changing anything.
