# EPIC001 Plan: Storage Behaviour and Memory Backend

## Progress

- [x] Phase 1.1: `BrainIndex.Storage` behaviour per ADR007.
- [x] Phase 1.2: `BrainIndex.Storage.Record`: frame/unframe with CRC32, decode errors.
- [x] Phase 1.3: `BrainIndex.Storage.Memory` (Agent-backed, reopenable for crash simulation).
- [x] Phase 1.4: Shared contract test module run against every backend.

## Quality Gate

- [x] Contract tests green for Memory.
- [x] Codec rejects bit flips and truncation.
- [x] `bin/qa_check.sh` passes.

## Commit Rule

`roadmap002 - epic {k} - {t.lower()}`
