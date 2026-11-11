# EPIC002 Spec: Collection Process

## Scope

- `BrainIndex.Collection` GenServer: start_link/child_spec, upsert, upsert_many, delete, search, info, checkpoint.
- Apply-then-append-then-ack; crash on append failure.
- Recovery: checkpoint + replay on init.
- Reads via `:persistent_term` without calling the process.

## Acceptance Criteria

- Restart (reopen Memory store) reproduces all acknowledged writes.
- Invalid writes are rejected and not logged.
- Searches never call the GenServer.
