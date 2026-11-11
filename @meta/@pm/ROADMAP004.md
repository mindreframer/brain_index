# ROADMAP004 — Compaction and Document-Level Retrieval

- **Status:** Completed (2026-11-11)
- **Target release:** `0.4.0` (internal)
- **ADRs:** ADR011 (compaction)

## Goal

Deleted vectors can be reclaimed, and the demo's retrieval returns distinct documents instead of
several chunks of the same document.

## Quality Bar

- Compaction keeps hits, filters and payloads identical (exact search), and reports
  `deleted: 0`.
- The demo's recall@5 does not regress. It is measured by `mix brain.eval` (document-level
  hits).
- The suite stays fast.

## Epics

| Epic | Outcome |
|---|---|
| EPIC001 | `BrainIndex.compact/3`: snapshot rebuild into a new directory (ADR011) |
| EPIC002 | Demo: group retrieval by document (over-fetch, best chunk per doc) |
