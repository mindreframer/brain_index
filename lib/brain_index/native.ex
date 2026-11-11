defmodule BrainIndex.Native do
  @moduledoc false

  # Precompiled NIFs are downloaded from the GitHub release of this version (ADR013).
  # BRAIN_INDEX_BUILD=1 builds from source instead (needs Rust; used by bin/qa_check.sh and CI).
  version = Mix.Project.config()[:version]

  use RustlerPrecompiled,
    otp_app: :brain_index,
    crate: "brain_index_nif",
    version: version,
    base_url: "https://github.com/mindreframer/brain_index/releases/download/nif-v#{version}",
    targets: ~w(
      aarch64-apple-darwin
      x86_64-apple-darwin
      aarch64-unknown-linux-gnu
      x86_64-unknown-linux-gnu
      aarch64-unknown-linux-musl
      x86_64-unknown-linux-musl
      x86_64-pc-windows-msvc
      x86_64-pc-windows-gnu
    ),
    nif_versions: ["2.15"],
    force_build: System.get_env("BRAIN_INDEX_BUILD") in ["1", "true"]

  def nif_version, do: err()
  def open(_dir, _config), do: err()
  def temp(_config), do: err()
  def insert_many(_index, _items), do: err()
  def search(_index, _query, _filter, _opts), do: err()
  def delete(_index, _id), do: err()
  def get_payload(_index, _id), do: err()
  def info(_index), do: err()
  def backup(_index, _dir), do: err()
  def compact(_index, _dir, _batch), do: err()

  defp err, do: :erlang.nif_error(:nif_not_loaded)
end
