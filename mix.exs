defmodule BrainIndex.MixProject do
  use Mix.Project

  @version "0.2.0"

  def project do
    [
      app: :brain_index,
      version: @version,
      elixir: "~> 1.18",
      elixirc_paths: elixirc_paths(Mix.env()),
      start_permanent: Mix.env() == :prod,
      description: "Embedded vector index with exact metadata filters (Rust NIF).",
      deps: deps()
    ]
  end

  defp elixirc_paths(:test), do: ["lib", "test/support"]
  defp elixirc_paths(_env), do: ["lib"]

  def application, do: [extra_applications: [:logger]]

  defp deps do
    [{:rustler, "== 0.38.0", runtime: false}]
  end
end
