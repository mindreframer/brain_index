for model <- [:e5, :v5] do
  defmodule Module.concat(BrainIndex.OracleTest, model |> to_string() |> String.upcase()) do
    @moduledoc "Recall on the spike fixtures must match the oracle numbers in ROADMAP001 §4."
    use ExUnit.Case, async: true
    alias BrainIndex.Fixtures

    @model model
    # Exact-search recall on the fixtures (114/150 and 101/150 queries).
    @oracle %{e5: %{plain: 0.76, filtered: 0.86}, v5: %{plain: 0.6733, filtered: 0.80}}[model]

    setup_all do
      {:ok, index} = BrainIndex.new(dim: Fixtures.dim(@model), metric: Fixtures.metric(@model))

      items =
        Enum.zip_with(Fixtures.chunks(), Fixtures.vectors(@model, :chunks), fn c, v ->
          {c.id, v, %{"kind" => c.kind, "lang" => c.lang}}
        end)

      :ok = BrainIndex.insert_many(index, items)

      %{index: index, queries: Fixtures.vectors(@model, :queries)}
    end

    test "exact search reproduces oracle plain recall", %{index: index, queries: qs} do
      results = Enum.map(qs, fn q -> elem(BrainIndex.search(index, q, 30, exact: true), 1) end)
      assert_in_delta Fixtures.recall_at_5(results), @oracle.plain, 0.001
    end

    test "hnsw search with default ef matches exact recall", %{index: index, queries: qs} do
      results = Enum.map(qs, fn q -> elem(BrainIndex.search(index, q, 30), 1) end)
      assert_in_delta Fixtures.recall_at_5(results), @oracle.plain, 0.001
    end

    test "kind filter reaches oracle filtered recall (exact and hnsw paths)", %{
      index: index,
      queries: qs
    } do
      for threshold <- [0, 4096] do
        results =
          Enum.zip(Fixtures.queries(), qs)
          |> Enum.map(fn {meta, q} ->
            {:ok, hits} =
              BrainIndex.search(index, q, 30,
                filter: %{"kind" => meta.kind},
                brute_force_threshold: threshold
              )

            hits
          end)

        assert Fixtures.recall_at_5(results) >= @oracle.filtered
      end
    end

    test "hnsw top-10 recall vs exact >= 0.98", %{index: index, queries: qs} do
      overlap =
        Enum.sum_by(qs, fn q ->
          {:ok, exact} = BrainIndex.search(index, q, 10, exact: true)
          {:ok, approx} = BrainIndex.search(index, q, 10)

          MapSet.size(
            MapSet.intersection(MapSet.new(exact, &elem(&1, 0)), MapSet.new(approx, &elem(&1, 0)))
          )
        end)

      assert overlap / (10 * length(qs)) >= 0.98
    end
  end
end
