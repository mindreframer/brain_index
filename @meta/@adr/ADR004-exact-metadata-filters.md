# ADR004: Exact-Match Metadata Filters with a Brute-Force Fallback

- Status: Accepted
- Date: 2026-11-11
- Decision owners: BrainIndex maintainers

## Context

Filtering by `kind` raised R@5 from 0.76 to 0.86. Agents also need
`lang` and `performance` filters (for example "only good outreach emails").
Filtered HNSW traversal is known to lose recall when a filter is very
selective, because the graph neighbourhood may contain no allowed nodes.
HyperspaceDB handles this with a brute-force path below a selectivity
threshold.

Keyword search (BM25) was measured as harmful for our queries. HyperspaceDB's
implementation also stems with only one language per server.

## Decision

- Each vector carries metadata: a map of string keys to string values. Only
  exact matching is supported.
- Filter semantics: `%{"kind" => "kb", "lang" => ["de", "en"]}` means
  `kind = kb AND lang IN (de, en)`. An empty filter means no filtering.
- Implementation: a posting bitset per `(key, value)` pair over internal ids.
  For a query, the bitsets are combined (OR within a key, AND across keys)
  and live-node tombstones are excluded.
- Strategy: if the allowed set has at most `brute_force_threshold` members
  (default 4,096, configurable per search), run exact brute force over the
  allowed ids. Otherwise run HNSW traversal, where every node is used for
  navigation but only allowed nodes enter the results, with `ef` raised to at
  least `k`.
- No BM25, keyword search or numeric range filters in this roadmap.

## Consequences

- Selective filters are *exact* (brute force), so they never lose recall.
- Memory per posting is one bit per node per distinct `(key, value)`. That's
  fine for low-cardinality fields. High-cardinality fields (for example a
  unique document id) should not be used as filter keys. This is documented.
- Keyword search, if ever needed, belongs to a separate component, for
  example Postgres full-text search or Tantivy, fused in the application.

## Alternatives Considered

- **Roaring bitmaps.** Better for sparse, high-cardinality postings, but adds
  a dependency. Dense bitsets are simpler and faster at our sizes. It can be
  swapped in behind the same internal interface.
- **Post-filtering only (search, then drop).** Rejected because it loses
  recall on selective filters.
- **Numeric range filters.** Deferred until a use case exists (for example
  "engagement_rate > 0.03"). Today, good and bad labels are categorical.
