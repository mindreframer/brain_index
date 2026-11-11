# EPIC006 Plan: Performance

## Progress

- [ ] Phase 6.1: SIMD kernels and equivalence tests.
- [ ] Phase 6.2: insert_many and binary vectors.
- [ ] Phase 6.3: Dirty schedulers and benchmark.
- [ ] Phase 6.4: QA; commit.

## Quality Gate

- [ ] SIMD and scalar kernels agree within 1e-5.
- [ ] Results identical to before (oracle tests unchanged).
- [ ] Documented benchmark numbers on fixtures.
- [ ] `bin/qa_check.sh` passes.

## Commit Rule

Commit as `roadmap001 - epic {k} - {title.lower()}` after all checks pass.
