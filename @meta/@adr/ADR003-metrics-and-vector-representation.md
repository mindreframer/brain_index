# ADR003: Metrics and Vector Representation

- Status: Accepted
- Date: 2026-11-11
- Decision owners: BrainIndex maintainers

## Context

Our current state of knowledge from the spike:

- `multilingual-e5-base` (768 dimensions, cosine) is the best measured model:
  R@5 0.87 filtered, 0.75 cross-lingual.
- YARlabs `v5_Embedding` (Lorentz, up to 129 dimensions) is 6× smaller, scores
  0.81 filtered, and is slightly better cross-lingual (0.79). Its marketing
  claim ("16D keeps 97% of the quality") did not hold, but hyperbolic models
  may improve, and they suit hierarchical data.
- Other HyperspaceDB metrics (L1, L∞, Jaccard, Hamming, Poincaré, Wasserstein,
  "hybrid 801D") have no current use case.

## Decision

Support exactly two metrics, chosen per index at creation:

- `:cosine`. Vectors are L2-normalised on insert and on query. The distance is
  `1 - dot(a, b)`, in the range [0, 2]. Zero vectors are rejected.
- `:lorentz`. Vectors are points on the hyperboloid, with time component
  `x0` first. Distance is `acosh(max(1, -<a,b>_L))`, where
  `<a,b>_L = -a0*b0 + Σ ai*bi`. Inputs must have `x0 >= 1`; they are stored
  as given.

Vectors are stored as contiguous `f32` in one arena indexed by internal id,
and distances are computed in `f32`. All vectors in an index share one fixed
dimension, set at creation.

## Consequences

- Adding a metric later is a local change (one enum variant and one kernel).
- `f32` halves memory compared with HyperspaceDB. The precision is
  sufficient: oracle recall must match exact-search results.
- Callers that change embedding models build a new index. There is no mixing.

## Alternatives Considered

- **Expose all 12 HyperspaceDB metrics.** Rejected as speculative; each one
  adds test surface.
- **`f64` storage.** Rejected: twice the memory, no measured benefit.
- **Quantization (int8 or ternary) now.** Deferred. The spike showed no recall
  loss for ternary, but RAM is not a constraint at current scale. It will come
  as a separate ADR when needed.
