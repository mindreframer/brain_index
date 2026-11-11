for model <- [:e5, :v5] do
  defmodule Module.concat(BrainIndex.OracleTest, model |> to_string() |> String.upcase()) do
    @moduledoc "Recall on the spike fixtures must match the oracle numbers in ROADMAP001 §4."
    use ExUnit.Case, async: true
    alias BrainIndex.Fixtures

    @model model
    @oracle %{e5: %{plain: 0.77}, v5: %{plain: 0.67}}[model]

    setup_all do
      {:ok, index} = BrainIndex.new(dim: Fixtures.dim(@model), metric: Fixtures.metric(@model))

      Enum.zip(Fixtures.chunks(), Fixtures.vectors(@model, :chunks))
      |> Enum.each(fn {c, v} -> :ok = BrainIndex.insert(index, c.id, v) end)

      %{index: index, queries: Fixtures.vectors(@model, :queries)}
    end

    test "exact search reproduces oracle plain recall", %{index: index, queries: qs} do
      results = Enum.map(qs, fn q -> elem(BrainIndex.search(index, q, 30), 1) end)
      assert_in_delta Fixtures.recall_at_5(results), @oracle.plain, 0.011
    end
  end
end
