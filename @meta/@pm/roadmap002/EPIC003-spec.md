# EPIC003 Spec: File Backend

## Scope

- Segmented append-only log with datasync per append.
- Torn-tail truncation on open; mid-log corruption -> error.
- Directory fsync NIF for new files and renames.
- Segment rotation at `segment_bytes`.

## Acceptance Criteria

- Contract tests green for File.
- Torn tail recovered; flipped byte in a closed segment -> corrupt_wal.
