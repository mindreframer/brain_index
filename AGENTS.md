# AGENTS.md

## Project

BrainIndex: an embedded vector index (Rust NIF) for the company brain. Internal package, not published.
Read `@meta/@adr/` before changing behaviour. A behaviour change needs an ADR (new or superseding).

## Roadmaps

- Work is roadmap-driven: `@meta/@pm/ROADMAP00X.md` + `roadmap00x/EPIC00N-{spec,plan}.md`.
- Execute epics in order. Every phase ships with tests. Check boxes only when code, tests and acceptance pass.
- After every epic run `bin/qa_check.sh` and fix all failures.
- Commit each green epic as `roadmap00x - epic x - <outcome>` with a short body: result + verification.

## Code

- Least abstraction that solves the current problem. No speculative features.
- Rust dependencies: `rustler`, `heed` (ADR009). Elixir dependencies: `rustler`, optional `objectstorex` (ADR012). Adding one needs an ADR.
- Oracle fixtures (`test/fixtures`) define correctness: recall thresholds in ROADMAP001 §4 must hold.
- Tests must stay fast (full suite < 10 s after compile). No sleeps, no network.
- `BrainIndex.Native` is private; the public API is `BrainIndex`.
