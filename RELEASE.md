# Release

Precompiled NIFs come from GitHub releases; there is no Hex publication (ADR013).

1. Bump `@version` in `mix.exs`, update `CHANGELOG.md`, run `bin/qa_check.sh`, commit, push.
2. `git tag nif-vX.Y.Z && git push origin nif-vX.Y.Z`. The NIF Release workflow builds the 8 targets
   and attaches them to the `nif-vX.Y.Z` release.
3. When all jobs are green: `mix gen.checksum` (writes `checksum-Elixir.BrainIndex.Native.exs`).
4. Commit the checksum, `git tag vX.Y.Z`, then push the commit and the tag.

Consumers: `{:brain_index, github: "mindreframer/brain_index", tag: "vX.Y.Z"}`. No Rust needed.
Build from source anyway: `BRAIN_INDEX_BUILD=1 mix deps.compile brain_index --force`.
