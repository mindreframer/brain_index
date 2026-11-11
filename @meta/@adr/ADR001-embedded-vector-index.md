# ADR001: Embedded In-Process Vector Index

- Status: Accepted
- Date: 2026-11-11
- Decision owners: BrainIndex maintainers

## Context

The company brain needs to retrieve, for each agent (support, social media,
outreach), the most relevant documents, guidelines and past examples from a
multilingual (EN/DE/FR/IT/ES) knowledge base. A spike (Nov 2026,
`../REPORT.md`) evaluated HyperspaceDB, an MIT-licensed Rust vector database,
on 427 synthetic company documents (543 chunks) and 150 labelled queries, 48%
of them cross-lingual.

Measured facts that shape this decision:

- The retrieval recipe that wins is **dense vectors plus exact-match metadata
  filters** (`kind`, `lang`, `performance`). With `multilingual-e5-base`,
  R@5 was 0.86 with a kind filter and 0.76 without. BM25 and hybrid fusion
  scored *lower* (0.43–0.47).
- HyperspaceDB's index crates embed cleanly in-process. They produced
  identical recall to the server, p50 search of 36–234 µs versus about 1 ms
  over gRPC, and a 3.4 MB binary.
- The HyperspaceDB server and SDK brought operational liabilities:
  - silent fallbacks that sent text to the vendor's cloud
  - hardcoded ports
  - a stale lock after a crash that blocked restarts
  - dropped connections without reconnect
  - about 49k lines of server code we would not use
- Expected scale is thousands to low millions of chunks per company. That fits
  comfortably in RAM on one node.

## Decision

BrainIndex is an Elixir library with an **embedded Rust vector index**,
loaded as a Rustler NIF. There is no server process, no network protocol and
no external service. The index lives inside the BEAM node that uses it.

Responsibilities:

- BrainIndex owns: vector storage, approximate nearest-neighbour search,
  metadata filtering, deletion, and snapshot persistence.
- The application owns: embeddings, chunking, documents and payload text,
  the system of record, retrieval policy, and replication across nodes.

## Consequences

### Positive

- Sub-millisecond search with no network hop and no service to operate.
- Full control of the code. It is small enough to understand completely and
  to change.
- Results are deterministic, tested against the spike's oracle numbers.

### Negative

- Index memory is inside the BEAM OS process, so it must be sized deliberately.
- Each node holds its own copy. Multi-node consistency is the application's
  job, for example rebuilding or loading snapshots on each node.
- Rust native code needs a Rust toolchain to build (internal package, no
  precompiled binaries).

## Alternatives Considered

### HyperspaceDB server + SDK

Rejected. See the liabilities above. Using its features would cost us its
operational baggage.

### Postgres + pgvector

A strong, boring option, and still the fallback if in-process memory becomes a
problem. Not chosen first because it requires a database round-trip per query,
gives less control over the filtered-search strategy and the Lorentz metric,
and the spike showed an embedded index is simple to own.

### Qdrant or another vector-database service

Rejected for the same operational reasons as the HyperspaceDB server. It is
also a second system of record.

## Verification

ROADMAP001 epic acceptance tests reproduce the spike's oracle recall from
Elixir, using real spike vectors as fixtures.
