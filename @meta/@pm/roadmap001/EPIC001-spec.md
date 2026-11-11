# EPIC001 Spec: Package Skeleton and QA Gate

## Purpose

Create a compiling Elixir package with a loading Rust NIF, a single QA script, and the spike oracle fixtures.

## Scope

- mix.exs with `rustler` as the only dependency; `native/brain_index_nif` crate with only `rustler`.
- `BrainIndex.Native` loads; `BrainIndex.Native.nif_version/0` smoke function.
- `bin/qa_check.sh`: format, compile with warnings as errors, ExUnit, cargo fmt/clippy/test.
- `test/fixtures`: binary f32 fixtures exported from the spike (e5 chunks/queries, v5 chunks/queries, labels).

## Acceptance Criteria

- `mix test` loads the NIF.
- Fixtures load in tests in under 100 ms.
- `bin/qa_check.sh` passes.
