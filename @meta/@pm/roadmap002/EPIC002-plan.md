# EPIC002 Plan: Collection Process

## Progress

- [ ] Phase 2.1: `BrainIndex.Collection` GenServer: start_link/child_spec, upsert, upsert_many, delete, search, info, checkpoint.
- [ ] Phase 2.2: Apply-then-append-then-ack; crash on append failure.
- [ ] Phase 2.3: Recovery: checkpoint + replay on init.
- [ ] Phase 2.4: Reads via `:persistent_term` without calling the process.

## Quality Gate

- [ ] Restart (reopen Memory store) reproduces all acknowledged writes.
- [ ] Invalid writes are rejected and not logged.
- [ ] Searches never call the GenServer.
- [ ] `bin/qa_check.sh` passes.

## Commit Rule

`roadmap002 - epic {k} - {t.lower()}`
