# EPIC003 Spec: Durability and Benchmarks

## Scope

- Crash test via separate OS process halting mid-run.
- Benchmark: build rate, p50/p99, reopen time, RSS, bytes written per insert.

## Acceptance Criteria

- Every acknowledged write survives; numbers in bench/RESULTS.md.
