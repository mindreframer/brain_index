# EPIC003 Plan: File Backend

## Progress

- [ ] Phase 3.1: Segmented append-only log with datasync per append.
- [ ] Phase 3.2: Torn-tail truncation on open; mid-log corruption -> error.
- [ ] Phase 3.3: Directory fsync NIF for new files and renames.
- [ ] Phase 3.4: Segment rotation at `segment_bytes`.

## Quality Gate

- [ ] Contract tests green for File.
- [ ] Torn tail recovered; flipped byte in a closed segment -> corrupt_wal.
- [ ] `bin/qa_check.sh` passes.

## Commit Rule

`roadmap002 - epic {k} - {t.lower()}`
