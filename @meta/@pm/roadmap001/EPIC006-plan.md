# EPIC006 Plan: Performance

## Progress

- [x] Phase 6.1: SIMD kernels and equivalence tests.
- [x] Phase 6.2: insert_many and binary vectors.
- [x] Phase 6.3: Dirty schedulers and benchmark.
- [x] Phase 6.4: QA; commit.

## Quality Gate

- [x] SIMD and scalar kernels agree within 1e-5.
- [x] Results identical to before (oracle tests unchanged).
- [x] Documented benchmark numbers on fixtures.
- [x] `bin/qa_check.sh` passes.

## Commit Rule

Commit as `roadmap001 - epic {k} - {title.lower()}` after all checks pass.
