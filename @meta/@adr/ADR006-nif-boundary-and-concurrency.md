# ADR006: NIF Boundary, Concurrency and Errors

- Status: Accepted
- Date: 2026-11-11
- Decision owners: BrainIndex maintainers

## Context

Native code runs inside the BEAM. Long NIF calls block schedulers, and panics
can crash the VM. Many agent processes will search concurrently, while writes
are comparatively rare (ingestion).

## Decision

- The index is a Rustler `ResourceArc` wrapping `RwLock<Index>`. Searches take
  a read lock and run in parallel. Inserts and deletes take the write lock.
- Every NIF that can take more than about 1 ms (`search`, `insert_many`,
  `save`, `load`) runs on a **dirty CPU scheduler**. Trivial calls (`size`,
  `info`) run on normal schedulers.
- Every NIF body is wrapped in `catch_unwind`. A panic becomes
  `{:error, %BrainIndex.Error{reason: :internal}}`.
- Errors are a stable `%BrainIndex.Error{reason: atom, message: binary}`.
  Reasons: `:invalid_argument`, `:dimension_mismatch`, `:invalid_vector`,
  `:io`, `:corrupt_snapshot`, `:internal`.
- Vectors cross the boundary either as a list of floats or as a binary of
  little-endian `f32`. The binary form is the fast path for bulk ingestion.
- `BrainIndex.Native` is private (`@moduledoc false`). The public API is
  `BrainIndex`.
- The index has no owner process: it is a plain term that can be shared and
  is garbage-collected when unreferenced. Supervision, if wanted, belongs to
  the application.

## Consequences

- Concurrent reads scale across cores. A large `insert_many` blocks searches
  for its duration. Callers batch writes accordingly. A future ADR may add
  copy-on-write snapshots if this becomes a problem.
- No GenServer bottleneck in the library.

## Alternatives Considered

- **GenServer owning the index.** Rejected: it serialises all searches
  through one process mailbox.
- **Yielding NIFs or timeslice-consumption NIFs.** Unnecessary: dirty
  schedulers are simpler and searches take microseconds to milliseconds.
