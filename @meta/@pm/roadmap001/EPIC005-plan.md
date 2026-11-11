# EPIC005 Plan: Mutations and Persistence

## Progress

- [x] Phase 5.1: Tombstones and upsert.
- [x] Phase 5.2: Snapshot writer and reader.
- [x] Phase 5.3: Corruption tests.
- [x] Phase 5.4: QA; commit.

## Quality Gate

- [x] Save → load → identical search results on the fixtures.
- [x] Flipped byte or truncated file gives `:corrupt_snapshot`, never a crash.
- [x] Deleted ids never appear; upsert changes the vector and metadata.
- [x] `bin/qa_check.sh` passes.

## Commit Rule

Commit as `roadmap001 - epic {k} - {title.lower()}` after all checks pass.
