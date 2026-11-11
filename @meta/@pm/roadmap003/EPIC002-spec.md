# EPIC002 Spec: Elixir API and Removal

## Scope

- `open/2`, `new/1`, payload in insert/insert_many, `payload: true` search, `get_payload/2`, `backup/2`.
- Delete snapshot.rs, Collection, Storage.*, Record and their tests.
- Port oracle/contract/concurrency tests.

## Acceptance Criteria

- Oracle thresholds hold; public API documented; no dead code.
