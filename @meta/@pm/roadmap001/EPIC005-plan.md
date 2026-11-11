# EPIC005 Plan: Mutations and Persistence

## Progress

- [ ] Phase 5.1: Tombstones and upsert.
- [ ] Phase 5.2: Snapshot writer and reader.
- [ ] Phase 5.3: Corruption tests.
- [ ] Phase 5.4: QA; commit.

## Quality Gate

- [ ] Save → load → identical search results on the fixtures.
- [ ] Flipped byte or truncated file gives `:corrupt_snapshot`, never a crash.
- [ ] Deleted ids never appear; upsert changes the vector and metadata.
- [ ] `bin/qa_check.sh` passes.

## Commit Rule

Commit as `roadmap001 - epic {k} - {title.lower()}` after all checks pass.
