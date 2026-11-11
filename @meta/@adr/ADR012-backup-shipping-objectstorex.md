# ADR012: Shipping Backups to Object Storage via objectstorex

- Status: Accepted. Implements the S3 part of ADR008.
- Date: 2026-11-11
- Decision owners: BrainIndex maintainers

## Context

`backup/2` produces a consistent, compacted single file (ADR009). It has to leave the machine.
We already own `objectstorex` (Rust `object_store` over Rustler, precompiled), which covers S3,
GCS and Azure, plus a **local folder** and memory with the same API. A folder is therefore a
working stand-in for a bucket, both in tests and on a single box.

## Decision

`BrainIndex.Shipper` (it exists only when the optional `:objectstorex` dependency is present):

- `ship(index, store, prefix)` runs `backup/2` into a temp directory, then streams `data.mdb` as
  one object `<prefix>/<UTC basic ISO timestamp>.mdb` using a multipart upload in 8 MiB chunks.
  The object appears complete or not at all.
- `list/2` (oldest first; keys sort by time), `latest/2`, `prune(store, prefix, keep: n)`.
- `restore(store, key, dir)` downloads to `data.mdb.part`, then renames. It refuses a directory
  that already holds an index.
- No manifest, no incremental uploads, no scheduler. The application decides when to call
  `ship` and `prune`.

## Consequences

- One code path for a folder, S3, R2, MinIO, GCS and Azure. Tests use a folder store with no
  credentials and no network.
- Each shipment is a full compacted copy. That's fine at our sizes (hundreds of MB), but
  incremental shipping would need a page-level design (ADR008 replication).
- `rustler` is pinned with `override: true`, because objectstorex asks for `~> 0.35`. It loads a
  precompiled NIF, so the override does not affect it.

## Alternatives Considered

- **ExAws / Req S3 directly:** S3 only, a second code path for local folders, and more
  dependencies.
- **A mandatory dependency:** forces a NIF download on users of the index alone. Kept optional.
