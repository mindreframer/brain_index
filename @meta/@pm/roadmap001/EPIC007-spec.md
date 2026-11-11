# EPIC007 Spec: API Hardening and Acceptance

## Purpose

A clean, documented public API that is safe at the boundary.

## Scope

- `BrainIndex` moduledocs, typespecs and option validation; `BrainIndex.Error`.
- Panic containment in every NIF (`catch_unwind`).
- `info/1` (dim, metric, size, deleted, params); `size/1`.
- README with usage, guarantees and limits; CHANGELOG; THIRD_PARTY_NOTICES.

## Acceptance Criteria

- End-to-end acceptance test: build from the fixtures through the public API, oracle thresholds met.
- Concurrency test: 8 parallel searchers plus a writer, no errors.
- Full QA passes; suite under 10 s.
