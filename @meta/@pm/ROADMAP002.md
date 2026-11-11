# ROADMAP002 — Durable Collections (Write-Ahead Log) and Demo Application

- **Status:** In progress
- **Target release:** `0.2.0` (internal)
- **ADRs:** ADR007 (WAL), ADR008 (deferred: quantization, S3, replication)

## 1. Product Goal

Acknowledged writes survive crashes, without periodic full snapshots of the whole index.
Set it up once:

```elixir
children = [
  {BrainIndex.Collection,
   name: :brain, dim: 768, storage: {BrainIndex.Storage.File, dir: "/var/lib/brain"}}
]

:ok = BrainIndex.Collection.upsert_many(:brain, [{1, vec, %{"kind" => "kb"}}])
{:ok, hits} = BrainIndex.Collection.search(:brain, query, 5, filter: %{"kind" => "kb"})
```

## 2. Quality Bar

- Crash matrix: after any simulated crash point (torn record, crash after checkpoint write,
  crash before segment deletion), recovery reproduces every acknowledged write.
- Memory and File backends pass the same behaviour test suite.
- The suite stays under 10 s. No sleeps.

## 3. Epics

| Epic | Outcome |
|---|---|
| EPIC001 | Storage behaviour, record codec, Memory backend, shared contract tests |
| EPIC002 | Collection process: apply → append → ack, recovery, `:persistent_term` reads |
| EPIC003 | File backend: segments, CRC framing, torn-tail repair, directory fsync |
| EPIC004 | Checkpoints: rotation, cleanup, auto-trigger, crash matrix |
| EPIC005 | Demo app `brain_demo`: local e5 embeddings, ingest, retrieval, LLM drafting |
