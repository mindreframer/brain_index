# EPIC007 Plan: API Hardening and Acceptance

## Progress

- [ ] Phase 7.1: Public API polish and validation.
- [ ] Phase 7.2: Panic guard and error contract tests.
- [ ] Phase 7.3: Docs, README, CHANGELOG, notices.
- [ ] Phase 7.4: Final QA; mark roadmap complete; commit.

## Quality Gate

- [ ] End-to-end acceptance test: build from the fixtures through the public API, oracle thresholds met.
- [ ] Concurrency test: 8 parallel searchers plus a writer, no errors.
- [ ] Full QA passes; suite under 10 s.
- [ ] `bin/qa_check.sh` passes.

## Commit Rule

Commit as `roadmap001 - epic {k} - {title.lower()}` after all checks pass.
