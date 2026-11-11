# EPIC005 Plan: Demo Application

## Progress

- [ ] Phase 5.1: `brain_demo` Mix project depending on brain_index by path.
- [ ] Phase 5.2: Local multilingual-e5 embeddings (ONNX).
- [ ] Phase 5.3: Ingest the spike corpus into a durable collection.
- [ ] Phase 5.4: Agent drafting (support / social / outreach) via an OpenAI-compatible LLM (Inworld DeepSeek).

## Quality Gate

- [ ] `mix brain.ingest` and `mix brain.ask` work end to end.
- [ ] Retrieval quality on the spike queries matches the oracle.
- [ ] `bin/qa_check.sh` passes.

## Commit Rule

`roadmap002 - epic {k} - {t.lower()}`
