# ADR009: LMDB Storage Engine (Disk-Resident, Incremental, No Snapshots)

- Status: Accepted. Supersedes ADR005 and ADR007, amends ADR001 and ADR002.
- Date: 2026-11-11
- Decision owners: BrainIndex maintainers

## Context

ADR007 (WAL plus periodic full checkpoints) still rewrites the whole index on every checkpoint.
It also keeps the whole index in RAM. Requirements, restated by the owner:

- **The data must not need to fit in memory.** The index must work when it is larger than RAM.
- **Respect the SSD.** Persisting a change must cost I/O proportional to the change. No periodic
  full dumps that wear the drive and use up I/O.
- Crash-safe, "set once and forget", testable.

Measured in a spike with `heed` 0.22 (LMDB) on Apple Silicon: 100k × 3 KiB values written and
committed in 0.34 s, random reads at about 1.1 µs each, and a clean build in 7 s with 14 crates
in total.

## Decision

The index lives in an **LMDB environment**, a memory-mapped copy-on-write B+tree, accessed
through the `heed` crate. One environment (a directory holding `data.mdb` and `lock.mdb`) holds
one index:

| table | key | value |
|---|---|---|
| `config` | name | dim, metric, m, ef_construction, seed, rng, entry, max_level, next_node |
| `vectors` | node u32 BE | dim × f32 LE (cosine: normalised) |
| `links` | node u32 BE + level u8 | neighbour node ids, u32 LE |
| `nodes` | node u32 BE | ext id u64, live flag, level count |
| `ids` | ext id u64 BE | node u32 (live nodes only) |
| `meta` | node u32 BE | metadata pairs |
| `payloads` | ext id u64 BE | opaque bytes (ADR010) |

- **Writes:** each `insert`, `insert_many` and `delete` call is one LMDB write transaction. Commit
  writes only the pages the transaction changed, then fsyncs. The caller is acknowledged only
  after the commit. A batch is one commit.
- **No WAL, no checkpoints, no snapshots.** LMDB never overwrites live pages; the new root is
  published atomically. After a crash the database is simply the last committed state: recovery
  takes zero time and there is nothing to replay. Readers left behind by a crashed process are
  cleared on open.
- **Reads:** each search runs in its own read transaction (an MVCC snapshot) and reads vectors and
  links straight from the memory map, with no copy into BEAM or Rust heaps. Searches never block
  writes, and writes never block searches.
- **Memory:** RAM holds only the OS page cache (hot pages, managed by the kernel) and the filter
  postings (one bit per node per distinct metadata pair, rebuilt by scanning `meta` on open). The
  data does not need to fit in RAM. Cold pages cost SSD random reads.
- **SIMD on mapped bytes:** distance kernels read stored vectors with unaligned loads, because
  LMDB values aren't guaranteed to be 4-byte aligned. Nothing is copied on the search hot path.
- **Map size:** an upper bound on file growth (default 1 TiB of address space; the file is sparse
  and grows with data).
- **Durability option:** `sync: true` (default) fsyncs on commit. `sync: false` is for tests and
  bulk rebuilds only.
- **Temporary indexes:** `BrainIndex.new/1` creates a private LMDB environment in a temp
  directory, which is deleted when the index is garbage-collected. There is one storage
  implementation for production and tests alike.
- **Backups:** `BrainIndex.backup(index, path)` makes a consistent, compacted online copy with
  `mdb_env_copy2`. This is the S3 unit for ADR008.
- **Deletes:** tombstones (the node stays for graph navigation; id, payload and postings are
  removed). Reclaiming tombstoned nodes is a future compaction step.

## Consequences

### Positive

- Persisting costs time proportional to the change. Disk writes are the changed B-tree pages.
- Indexes larger than RAM work. Reopening is instant (memory map plus postings scan).
- Removes the WAL, checkpoints, the snapshot format, the `Collection` process and the
  `Storage` behaviour. Net code shrinks.
- Crash safety is LMDB's, which is about 20 years old and proven in OpenLDAP, Meilisearch and
  Monero.

### Negative

- A new dependency (`heed`, which builds LMDB's C code). That's an exception to "rustler only"
  (ADR002), justified here.
- Copy-on-write B-tree write amplification: one insert rewrites the pages on the path to each
  changed key (about 30 keys per HNSW insert). Batching (`insert_many`) amortises this.
- One writer at a time, which matches our model (ADR006).
- Tombstones accumulate until compaction exists.
- Filter postings stay in RAM (small: 1M nodes × 10 pairs is about 1.25 MiB).

## Measured (bench/RESULTS.md, 10k × 384, Apple Silicon)

- Batched build: about 2× write amplification (30 MiB written for 14.6 MiB of raw vectors).
- A lone insert costs about 520 KB written (16 KiB pages × the B-tree paths of about 30 touched
  link lists). Hence `BrainIndex.Writer` (group commit): 43 KB per insert with 50 concurrent
  callers.
- Search p50 is about 1.1–1.4 ms (in-memory engine: 0.27 ms). Cold reopen takes 2 ms.

## Alternatives Considered

- **WAL plus checkpoints (ADR007).** Rejected: periodic full rewrites and RAM residency.
- **redb (pure-Rust copy-on-write B-tree).** A credible alternative with no C code. LMDB was
  chosen for maturity, zero-copy reads and its use as a vector-store backend (Meilisearch's
  arroy).
- **RocksDB (LSM tree).** Lower write amplification for random writes, but heavy (C++, about
  30 MB build), compaction stalls, and many settings to tune. Not "set and forget".
- **SQLite.** Excellent general-purpose store, but row-oriented access adds overhead per vector
  and link lookup on the HNSW hot path. It can remain the application's document store if wanted.
- **Hand-written page store with an mmap'd arena.** Rejected: we'd be rebuilding LMDB, badly.
