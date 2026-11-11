# EPIC004 Plan: Checkpoints

## Progress

- [ ] Phase 4.1: Checkpoint = rotate, atomic snapshot, delete obsolete files.
- [ ] Phase 4.2: Auto checkpoint when log exceeds `checkpoint_bytes`.
- [ ] Phase 4.3: Crash matrix tests for every checkpoint step.

## Quality Gate

- [ ] All crash points recover every acknowledged write.
- [ ] Obsolete segments are removed after a checkpoint.
- [ ] `bin/qa_check.sh` passes.

## Commit Rule

`roadmap002 - epic {k} - {t.lower()}`
