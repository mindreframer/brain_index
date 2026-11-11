# ADR010: Opaque Payloads Stored with Vectors

- Status: Accepted (amends ADR001's "application owns payloads")
- Date: 2026-11-11
- Decision owners: BrainIndex maintainers

## Context

The demo stored chunk text in DETS next to the index. That meant two stores and two failure
modes, no atomicity between a vector and its text, and DETS is not trusted (repair after an
unclean shutdown, a 2 GB file limit). Every retrieval needs the text of its hits.

## Decision

Each id may carry an opaque binary **payload**. It is written in the same LMDB transaction as the
vector and deleted with it (ADR009).

- `insert(index, id, vector, meta, payload)`; items in `insert_many` may be
  `{id, vector, meta, payload}`.
- `search(..., payload: true)` returns `{id, distance, payload}`.
- `get_payload(index, id)`.
- BrainIndex never interprets payloads. Applications choose the encoding (JSON or
  `:erlang.term_to_binary`).

## Consequences

- One store, atomic vector+text updates, and a consistent backup.
- Payloads live outside the search hot path: they are read only for the final hits.
- BrainIndex is still not a document database: no queries on payload content.

## Alternatives Considered

- **SQLite (exqlite) in the application.** Fine for rich document queries, but a second store
  without atomic updates. It stays an option for application data beyond retrieval.
- **DETS.** Rejected (see Context).
