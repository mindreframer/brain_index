# ADR011: Compaction by Rebuild Into a New Directory

- Status: Accepted. Amends ADR009 (deletes).
- Date: 2026-11-11
- Decision owners: BrainIndex maintainers

## Context

Deletes are tombstones (ADR009): the node keeps routing searches, but it still uses disk space
and makes searches walk through dead nodes. Without a way to reclaim them, an index with churn,
such as re-ingested documents, grows without bound.

## Decision

`BrainIndex.compact(index, dir, batch: 1000)` streams the live nodes (vector, metadata, payload)
from one read snapshot into a fresh index in the empty directory `dir`, in batches of `batch`.
That index keeps the source's `dim`, `metric`, `m` and `ef_construction`. The graph is rebuilt
from scratch, so it holds no tombstones and no repair heuristics are needed.

- The source stays online for searches and writes. Writes made after the snapshot are not copied.
  The application pauses writers, or replays them, before switching over.
- The target is built without fsync per batch and synced once at the end, so the cost is one
  sequential build.
- The target must be empty and must not be open in this OS process.
- Switching over (closing the old index and pointing the application at `dir`) is the
  application's job. The library never renames or deletes directories in use.

## Consequences

- Disk usage and graph quality return to those of a fresh build. Costs about one build: about
  750 vectors/s at 384D (bench/RESULTS.md).
- Peak disk usage is about twice the live data while both copies exist.
- There is no in-place graph repair, which keeps the code small. Running compaction is rare, so
  the operator decides when, using `info/1` `deleted`.

## Alternatives Considered

- **In-place repair on delete** (reconnecting neighbours and freeing node ids): more code in the
  hot write path, harder to keep crash-safe, and still fragments node ids.
- **Automatic background compaction:** hidden I/O spikes and an automatic directory swap, which
  goes against "set once and forget" and ADR009's I/O budget.
