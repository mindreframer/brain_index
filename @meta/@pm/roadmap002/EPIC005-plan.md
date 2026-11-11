# EPIC005 Plan: Demo Application

## Progress

- [x] Phase 5.1: `brain_demo` Mix project depending on brain_index by path.
- [x] Phase 5.2: Local multilingual-e5 embeddings (ONNX).
- [x] Phase 5.3: Ingest the spike corpus into a durable collection.
- [x] Phase 5.4: Agent drafting (support / social / outreach) via an OpenAI-compatible LLM (Inworld DeepSeek).

## Quality Gate

- [x] `mix brain.ingest` and `mix brain.ask` work end to end.
- [x] Retrieval quality on the spike queries matches the oracle.
- [x] `bin/qa_check.sh` passes.

## Commit Rule

`roadmap002 - epic {k} - {t.lower()}`
