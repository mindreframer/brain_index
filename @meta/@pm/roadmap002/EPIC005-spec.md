# EPIC005 Spec: Demo Application

## Scope

- `brain_demo` Mix project depending on brain_index by path.
- Local multilingual-e5 embeddings (ONNX).
- Ingest the spike corpus into a durable collection.
- Agent drafting (support / social / outreach) via an OpenAI-compatible LLM (Inworld DeepSeek).

## Acceptance Criteria

- `mix brain.ingest` and `mix brain.ask` work end to end.
- Retrieval quality on the spike queries matches the oracle.
