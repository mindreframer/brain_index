#!/usr/bin/env bash
# Single quality gate. Run after every epic.
set -euo pipefail
export BRAIN_INDEX_BUILD=1 # always test the local Rust source, never a downloaded binary
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
stage() { printf '\n[qa/%s] %s\n' "$1" "$2"; }
crate=native/brain_index_nif/Cargo.toml

stage elixir "format, compile"
mix format --check-formatted
MIX_ENV=test mix compile --warnings-as-errors

stage elixir "ExUnit"
MIX_ENV=test mix test

stage rust "format"
cargo fmt --manifest-path "$crate" --all -- --check

stage rust "clippy"
cargo clippy --manifest-path "$crate" --all-targets -- -D warnings

stage rust "tests"
cargo test --manifest-path "$crate" --release

stage ok "all checks passed"
