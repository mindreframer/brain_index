# EPIC002 Spec — Document-Level Retrieval (demo)

- `BrainDemo.Brain.search_docs(vec, k, filter)` returns at most `k` chunks with distinct
  `doc_id`s, each one the closest chunk of its document, ordered by distance.
- Agents (`Retrieval.context`), the LiveView search page and `mix brain.eval` use it.
- Recall@5 on the 150 queries must not regress (it stays at 0.80 plain and 0.893 with the kind filter).
