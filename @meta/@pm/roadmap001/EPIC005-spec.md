# EPIC005 Spec: Mutations and Persistence

## Purpose

Upsert, delete and crash-safe snapshots per ADR005.

## Scope

- Tombstones: deleted nodes navigate but never match; delete returns whether the id existed.
- Upsert = tombstone the old node + insert a new node; metadata postings updated.
- Snapshot format `BRIX` v1 with FNV-1a 64 checksum; atomic tmp+fsync+rename.
- `save/2`, `load/1`; corrupt or truncated files return `:corrupt_snapshot`.

## Acceptance Criteria

- Save → load → identical search results on the fixtures.
- Flipped byte or truncated file gives `:corrupt_snapshot`, never a crash.
- Deleted ids never appear; upsert changes the vector and metadata.
