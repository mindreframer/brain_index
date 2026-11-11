# EPIC004 Spec: Checkpoints

## Scope

- Checkpoint = rotate, atomic snapshot, delete obsolete files.
- Auto checkpoint when log exceeds `checkpoint_bytes`.
- Crash matrix tests for every checkpoint step.

## Acceptance Criteria

- All crash points recover every acknowledged write.
- Obsolete segments are removed after a checkpoint.
