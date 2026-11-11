# EPIC002 Plan: Elixir API and Removal

## Progress

- [x] Phase 2.1: `open/2`, `new/1`, payload in insert/insert_many, `payload: true` search, `get_payload/2`, `backup/2`.
- [x] Phase 2.2: Delete snapshot.rs, Collection, Storage.*, Record and their tests.
- [x] Phase 2.3: Port oracle/contract/concurrency tests.

## Quality Gate

- [x] Oracle thresholds hold; public API documented; no dead code.
- [x] `bin/qa_check.sh` passes.

## Commit Rule

`roadmap003 - epic {k} - {t.lower()}`
