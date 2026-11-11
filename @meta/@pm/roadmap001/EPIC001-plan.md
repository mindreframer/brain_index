# EPIC001 Plan: Package Skeleton and QA Gate

## Progress

- [x] Phase 1.1: Create mix project files and crate.
- [x] Phase 1.2: Add the NIF smoke function and test.
- [x] Phase 1.3: Write the fixture exporter and commit the fixtures.
- [x] Phase 1.4: Write the QA script and AGENTS.md; pass QA and commit.

## Quality Gate

- [x] `mix test` loads the NIF.
- [x] Fixtures load in tests in under 100 ms.
- [x] `bin/qa_check.sh` passes.
- [x] `bin/qa_check.sh` passes.

## Commit Rule

Commit as `roadmap001 - epic {k} - {title.lower()}` after all checks pass.
