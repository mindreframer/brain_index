defmodule BrainIndex.MixProject do
  use Mix.Project

  @version "0.5.0"
  @source_url "https://github.com/mindreframer/brain_index"
  @force_build? System.get_env("BRAIN_INDEX_BUILD") in ["1", "true"]

  def project do
    [
      app: :brain_index,
      version: @version,
      elixir: "~> 1.18",
      elixirc_paths: elixirc_paths(Mix.env()),
      start_permanent: Mix.env() == :prod,
      description: "Embedded vector index with exact metadata filters (Rust NIF over LMDB).",
      source_url: @source_url,
      package: package(),
      aliases: aliases(),
      deps: deps()
    ]
  end

  defp elixirc_paths(:test), do: ["lib", "test/support"]
  defp elixirc_paths(_env), do: ["lib"]

  def application, do: [extra_applications: [:logger]]

  defp deps do
    [
      # Precompiled NIFs come from GitHub releases (ADR013); Rust is only needed to build locally.
      {:rustler_precompiled, "~> 0.8"},
      {:rustler, "== 0.38.0", optional: not @force_build?, runtime: false, override: true},
      # Optional: only `BrainIndex.Shipper` (ADR012) needs it.
      {:objectstorex, "~> 0.3.0", optional: true}
    ]
  end

  defp package do
    [
      files: [
        "lib",
        "native/brain_index_nif/src",
        "native/brain_index_nif/Cargo.toml",
        "native/brain_index_nif/Cargo.lock",
        "native/brain_index_nif/Cross.toml",
        "rust-toolchain.toml",
        "mix.exs",
        "README.md",
        "LICENSE",
        "THIRD_PARTY_NOTICES.md",
        "CHANGELOG.md",
        "checksum-Elixir.BrainIndex.Native.exs"
      ],
      licenses: ["MIT"],
      links: %{"GitHub" => @source_url}
    ]
  end

  defp aliases do
    ["gen.checksum": "rustler_precompiled.download BrainIndex.Native --all --print"]
  end
end
