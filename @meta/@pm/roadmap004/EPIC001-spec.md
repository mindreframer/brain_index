# EPIC001 Spec — Compaction

- API: `BrainIndex.compact(index, dir, batch: 1000) :: {:ok, copied} | {:error, Error.t()}`.
- `dir` is created if missing. It must be empty and not open; otherwise the call returns
  `:invalid_argument`.
- The result keeps `dim`, `metric`, `m` and `ef_construction`, has `deleted: 0`, and returns the
  same exact hits, payloads and filter results as the source.
- Memory is bounded by `batch`. The target is fsynced once at the end.
