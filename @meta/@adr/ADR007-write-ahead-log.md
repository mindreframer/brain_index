# ADR007: Write-Ahead Log with Checkpoints

- Status: Superseded by ADR009
- Date: 2026-11-11
- Decision owners: BrainIndex maintainers

## Context

ADR005 made persistence an explicit full snapshot. That is fine for small indexes, but every
persist rewrites the whole index. As data grows, that becomes slow, I/O-heavy and easy to
forget, and anything written since the last snapshot is lost on a crash. The company brain
needs durability it can set once and forget: every acknowledged write must survive a crash,
and persisting must cost time proportional to the change, not to the index.

Constraints:

- Durability must be testable and mockable. Tests use an in-memory log; production uses files.
- No new dependencies (AGENTS.md).
- The design must not block later work: S3 upload and download, and replication (ADR008).

## Decision

### Durable write path: `BrainIndex.Collection`

A `Collection` is a supervised process that owns one index and one storage backend.

1. Writes (`upsert`, `upsert_many`, `delete`) go through the process, one at a time, in order.
2. For each write the process:
   1. validates the write and applies it to the in-memory index (invalid input is rejected and
      nothing is logged);
   2. appends a log record with the next **LSN** (log sequence number);
   3. syncs to disk;
   4. replies `:ok`.

   The caller is acknowledged only after the record is durable.
3. If appending fails after the index changed, the process crashes. Its supervisor restarts it,
   and recovery rebuilds the index from disk, so memory and disk can never disagree.
4. Searches do not go through the process. The index reference is published in
   `:persistent_term` and searched directly, concurrently (ADR006). A search can see a write a
   few microseconds before that write is acknowledged.

### Storage behaviour: `BrainIndex.Storage`

```elixir
@callback open(opts) :: {:ok, state} | {:error, term}
@callback load_checkpoint(state) :: {:ok, BrainIndex.t(), lsn} | :none | {:error, term}
@callback replay(state, after_lsn, acc, (record, acc -> acc)) :: {:ok, acc, last_lsn, state} | {:error, term}
@callback append(state, [record]) :: {:ok, state} | {:error, term}  # durable on return
@callback checkpoint(state, BrainIndex.t(), lsn) :: {:ok, state} | {:error, term}
@callback log_bytes(state) :: non_neg_integer
@callback close(state) :: :ok
```

A record is `{lsn, op}`, where `op` is `{:upsert, id, f32_binary, meta_pairs}` or
`{:delete, id}`.

- `BrainIndex.Storage.Memory`: keeps records and the checkpoint in an Agent. It is for tests,
  and for simulating crashes by reopening the same Agent.
- `BrainIndex.Storage.File`: the production backend.

### File backend layout

```text
dir/
  checkpoint-00000000000000001234.brix   # BRIX snapshot containing every op with LSN <= 1234
  wal-00000000000000001235.log           # segment whose first record has LSN 1235
  wal-00000000000000005001.log
```

- **Record framing:** `<<size::32, crc32::32, payload::binary-size(size)>>`, where the payload
  is `:erlang.term_to_binary({lsn, op})`. Each append writes all of its records with one write
  and syncs once with `datasync`. `upsert_many` is one batch and therefore one sync.
- **Segments** rotate when they exceed `segment_bytes` (default 64 MiB). New segment files and
  checkpoint renames are made durable with a directory fsync.
- **Torn tail:** a crash during an append can leave a partial record at the end of the *last*
  segment. On open, that tail is truncated and recovery continues. A bad record anywhere else is
  real corruption and fails loudly with `{:error, {:corrupt_wal, file, offset}}`.
- **Checkpoint:**
  1. rotate to a new segment;
  2. write `checkpoint-<lsn>.brix` atomically (tmp file, fsync, rename, directory fsync);
  3. delete older checkpoints and every segment whose records are all at or below the
     checkpoint LSN.

  A crash at any step leaves a recoverable state, because recovery uses the newest valid
  checkpoint and replays only records with a higher LSN.
- **Recovery** (on start): load the newest checkpoint (or start empty), then replay the
  segments in LSN order.

### Checkpoint policy

A checkpoint runs automatically when the log exceeds `checkpoint_bytes` (default 256 MiB), and
`Collection.checkpoint/1` forces one. The cost of a checkpoint is proportional to the index
size, but it happens rarely. Between checkpoints, each write costs one log append, so cost is
proportional to the change. Searches continue during a checkpoint, because it only takes a read
lock. Writes wait until it finishes.

### Defaults: "set once and forget"

```elixir
{BrainIndex.Collection, name: :brain, dim: 768, storage: {BrainIndex.Storage.File, dir: "/var/lib/brain"}}
```

Everything else has a safe default: sync on every write, 64 MiB segments, 256 MiB checkpoint
threshold.

## Consequences

### Positive

- Acknowledged writes survive crashes and power loss (with fsync).
- Persisting costs time proportional to the change. Full snapshots happen rarely and
  automatically.
- Tests use `Storage.Memory`. Crash scenarios can be tested deterministically for both
  backends.
- Immutable segments and checkpoints are exactly the units that S3 shipping and replication will
  move later (ADR008).

### Negative

- Writes are serialised through one process, and each one syncs to disk. Throughput depends on
  disk sync latency, so batch writes with `upsert_many`.
- A checkpoint still writes the whole index, just rarely. If indexes grow to tens of GB,
  incremental checkpoints would be needed. That would require a new ADR.
- Restart time is the checkpoint load plus the replay of up to `checkpoint_bytes` of log.

## Alternatives Considered

- **Log inside Rust.** Rejected: the BEAM owns supervision, and Elixir I/O with `datasync` is
  enough. The storage behaviour is easier to mock in Elixir. Rust keeps only what Elixir can't
  do: snapshot encoding and the directory fsync.
- **Snapshots only (ADR005 as is).** Rejected for the reasons above.
- **Embedded KV store (RocksDB, sled, redb) as the log.** Rejected: a large dependency for an
  append-only log. HyperspaceDB's redb lock incident in the spike is a cautionary data point.
- **Fsync batching on a timer (group commit).** Deferred. It would trade a small loss window for
  throughput. Explicit batches (`upsert_many`) get most of the benefit with no loss window.
