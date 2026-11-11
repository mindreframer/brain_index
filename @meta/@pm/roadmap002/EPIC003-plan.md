# EPIC003 Plan: File Backend

## Progress

- [x] Phase 3.1: Segmented append-only log with datasync per append.
- [x] Phase 3.2: Torn-tail truncation on open; mid-log corruption -> error.
- [x] Phase 3.3: Directory fsync NIF for new files and renames.
- [x] Phase 3.4: Segment rotation at `segment_bytes`.

## Quality Gate

- [x] Contract tests green for File.
- [x] Torn tail recovered; flipped byte in a closed segment -> corrupt_wal.
- [x] `bin/qa_check.sh` passes.

## Commit Rule

`roadmap002 - epic {k} - {t.lower()}`
