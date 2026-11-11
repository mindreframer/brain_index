# EPIC004 Plan: Checkpoints

## Progress

- [x] Phase 4.1: Checkpoint = rotate, atomic snapshot, delete obsolete files.
- [x] Phase 4.2: Auto checkpoint when log exceeds `checkpoint_bytes`.
- [x] Phase 4.3: Crash matrix tests for every checkpoint step.

## Quality Gate

- [x] All crash points recover every acknowledged write.
- [x] Obsolete segments are removed after a checkpoint.
- [x] `bin/qa_check.sh` passes.

## Commit Rule

`roadmap002 - epic {k} - {t.lower()}`
