# EPIC003 Plan: Durability and Benchmarks

## Progress

- [x] Phase 3.1: Crash test via separate OS process halting mid-run.
- [x] Phase 3.2: Benchmark: build rate, p50/p99, reopen time, RSS, bytes written per insert.

## Quality Gate

- [x] Every acknowledged write survives; numbers in bench/RESULTS.md.
- [x] `bin/qa_check.sh` passes.

## Commit Rule

`roadmap003 - epic {k} - {t.lower()}`
