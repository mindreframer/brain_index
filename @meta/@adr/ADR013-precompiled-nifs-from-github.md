# ADR013: Precompiled NIFs from GitHub Releases (No Hex Publication)

- Status: Accepted. Amends ADR002 (dependencies).
- Date: 2026-11-11
- Decision owners: BrainIndex maintainers

## Context

Hosts that use BrainIndex, such as a CPU-only server, should not need a Rust toolchain or a
multi-minute LMDB and LTO build. We want the convenience of our published libraries
(objectstorex) without publishing to Hex yet.

## Decision

- The repository `github.com/mindreframer/brain_index` is public. A `nif-v<version>` tag triggers
  `.github/workflows/nif-release.yml`, which builds the NIF for the same target matrix as
  objectstorex (macOS arm64/x86_64, Linux gnu/musl arm64/x86_64, Windows msvc/gnu) at NIF 2.15
  and attaches the files to the GitHub release.
- `BrainIndex.Native` uses `RustlerPrecompiled` with `base_url` pointing at that release. The
  checksum file `checksum-Elixir.BrainIndex.Native.exs` (`mix gen.checksum`) is committed after
  the release build.
- Consumers depend on Git: `{:brain_index, github: "mindreframer/brain_index", tag: "v0.5.0"}` (or a
  ref that contains the checksum file). There's no Hex package for now; `package/0` is ready if
  we change our mind.
- `BRAIN_INDEX_BUILD=1` forces a source build. `bin/qa_check.sh` and CI always set it, so tests
  run the checked-out Rust code, never a downloaded binary.
- SIMD stays runtime-dispatched (AVX2+FMA detection), so one generic x86_64 binary is fast on
  modern CPUs and correct on old ones.

## Consequences

- Two tags per release, so the consumer tag can contain the checksum: `nif-v<version>` (binaries)
  and `v<version>`, placed on the checksum commit. Flow: bump `@version` → push tag `nif-vX` → wait
  for the matrix → `mix gen.checksum` → commit → tag `vX`. See RELEASE.md.
- `rustler` becomes optional for consumers. `rustler_precompiled` is a runtime dependency.
