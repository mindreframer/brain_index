# EPIC002 Plan — Document-Level Retrieval (demo)

1. `search_docs`: over-fetch `max(6k, 30)` chunks, `uniq_by(doc_id)`, take `k`.
2. Switch retrieval, the UI and eval to it. Test: 3 chunks of one document plus one other document;
   expect the closest chunk of each.
3. Make the test setup independent of module order (ingest is resumable).
4. Real eval: 0.80 / 0.893, unchanged.
