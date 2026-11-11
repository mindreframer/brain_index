# EPIC001 Spec: Storage Behaviour and Memory Backend

## Scope

- `BrainIndex.Storage` behaviour per ADR007.
- `BrainIndex.Storage.Record`: frame/unframe with CRC32, decode errors.
- `BrainIndex.Storage.Memory` (Agent-backed, reopenable for crash simulation).
- Shared contract test module run against every backend.

## Acceptance Criteria

- Contract tests green for Memory.
- Codec rejects bit flips and truncation.
