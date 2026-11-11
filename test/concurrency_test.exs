defmodule BrainIndex.ConcurrencyTest do
  use ExUnit.Case, async: true

  test "parallel searchers and a writer share one index safely" do
    {:ok, index} = BrainIndex.new(dim: 16)
    vec = fn i -> for j <- 1..16, do: :math.sin(i * j) end
    :ok = BrainIndex.insert_many(index, for(i <- 1..500, do: {i, vec.(i), %{"p" => rem(i, 2)}}))

    writer =
      Task.async(fn ->
        for i <- 501..800, do: :ok = BrainIndex.insert(index, i, vec.(i), %{"p" => rem(i, 2)})
        for i <- 1..100, do: {:ok, true} = BrainIndex.delete(index, i)
      end)

    searchers =
      for s <- 1..8 do
        Task.async(fn ->
          for i <- 1..150 do
            {:ok, hits} = BrainIndex.search(index, vec.(i * s), 5, filter: %{"p" => rem(i, 2)})
            assert length(hits) == 5
          end
        end)
      end

    Task.await_many([writer | searchers], 30_000)
    assert BrainIndex.size(index) == 700
  end
end
