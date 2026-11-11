# EPIC001 Plan — Compaction

1. `Index::compact_to(dir, batch)`: one read txn, iterate `ids` (live only), and push
   `(ext, vector, meta, payload)` into `insert_many` batches on a `sync: false` target, then
   `force_sync`. Rust test: 200 vectors, 10 deleted, batch 7. Exact hits, filtered hits and
   payloads must match.
2. NIF `compact/3` (DirtyIo). It refuses targets that are registered as open and holds the
   registry lock during the build.
3. `BrainIndex.compact/3` plus an ExUnit test (deleted 10 → 0, filtered payload hits equal, busy
   target refused).
4. ADR011, README, CHANGELOG, version 0.4.0.
