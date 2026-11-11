# EPIC002 Plan: Exact Search

## Progress

- [x] Phase 2.1: Implement metrics and unit tests.
- [x] Phase 2.2: Implement arena, id maps and exact search.
- [x] Phase 2.3: Expose the NIFs and the Elixir wrapper.
- [x] Phase 2.4: Oracle test; QA; commit.

## Quality Gate

- [x] Rust unit tests for the metrics against hand-computed values.
- [x] Elixir oracle test: e5 brute force R@5 = 0.77 plain; v5 Lorentz R@5 = 0.67 plain (within 0.01).
- [x] Dimension and invalid-vector errors are returned, not raised from Rust.
- [x] `bin/qa_check.sh` passes.

## Commit Rule

Commit as `roadmap001 - epic {k} - {title.lower()}` after all checks pass.
