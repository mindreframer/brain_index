# ROADMAP003 — Disk-Resident LMDB Engine, Payloads, Live Demo UI

- **Status:** In progress
- **Target release:** `0.3.0` (internal)
- **ADRs:** ADR009 (LMDB engine, supersedes ADR005/ADR007), ADR010 (payloads)

## Goal

The index lives on disk in LMDB: changes cost I/O proportional to the change, the data does not
need to fit in RAM, and recovery is instant. One API serves temporary and persistent indexes.

```elixir
{:ok, index} = BrainIndex.open("/var/lib/brain", dim: 768)            # durable; reopen = recover
:ok = BrainIndex.insert_many(index, [{1, vec, %{"kind" => "kb"}, chunk_json}])
{:ok, [{1, 0.12, chunk_json} | _]} = BrainIndex.search(index, q, 5, filter: %{"kind" => "kb"}, payload: true)
```

## Quality Bar

- Oracle recall unchanged (ROADMAP001 §4).
- A real crash test: an OS process writes, then halts without cleanup; reopening sees every
  acknowledged write.
- Bytes written per single insert are reported by the benchmark, and bounded (no full rewrites).
- Suite under 10 s.

## Epics

| Epic | Outcome |
|---|---|
| EPIC001 | LMDB engine in Rust: tables, HNSW over transactions, postings rebuild, unaligned SIMD |
| EPIC002 | Elixir API: open/new/insert(payload)/search(payload)/delete/backup; remove WAL, snapshot, Collection |
| EPIC003 | Crash, durability and benchmark (RSS, write bytes per insert, reopen time) |
| EPIC004 | Demo: payloads instead of DETS, long-running app, faster model startup |
| EPIC005 | Phoenix LiveView UI: query, role/filters, results, LLM drafts, eval page |
