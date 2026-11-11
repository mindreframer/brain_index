defmodule BrainIndex.ApiContractTest do
  use ExUnit.Case, async: true
  alias BrainIndex.Error

  setup do
    {:ok, index} = BrainIndex.new(dim: 2, m: 8, ef_construction: 50)
    %{index: index}
  end

  test "info reports configuration and counters", %{index: index} do
    :ok = BrainIndex.insert_many(index, [{1, [1.0, 0.0]}, {2, [0.0, 1.0], %{kind: :kb}}])
    {:ok, true} = BrainIndex.delete(index, 1)

    assert BrainIndex.info(index) ==
             %{dim: 2, metric: :cosine, size: 1, deleted: 1, m: 8, ef_construction: 50}
  end

  test "insert_many is all-or-nothing", %{index: index} do
    assert {:error, %Error{reason: :invalid_vector, message: "item 1: " <> _}} =
             BrainIndex.insert_many(index, [{1, [1.0, 0.0]}, {2, [0.0, 0.0]}])

    assert {:error, %Error{reason: :invalid_argument, message: "item 0: " <> _}} =
             BrainIndex.insert_many(index, [{-1, [1.0, 0.0]}])

    assert BrainIndex.size(index) == 0
  end

  test "invalid arguments return tagged errors", %{index: index} do
    for {call, reason} <- [
          {fn -> BrainIndex.insert(index, -1, [1.0, 0.0]) end, :invalid_argument},
          {fn -> BrainIndex.insert(index, 2 ** 64, [1.0, 0.0]) end, :invalid_argument},
          {fn -> BrainIndex.insert(index, 1, [1.0, :x]) end, :invalid_vector},
          {fn -> BrainIndex.insert(index, 1, [1.0, 0.0], [:not_a_map]) end, :invalid_argument},
          {fn -> BrainIndex.search(index, [1.0, 0.0], 0) end, :invalid_argument},
          {fn -> BrainIndex.search(index, [1.0, 0.0], 1, ef: -1) end, :invalid_argument},
          {fn -> BrainIndex.search(index, [1.0, 0.0], 1, filter: "kind") end, :invalid_argument},
          {fn -> BrainIndex.new(dim: 4, m: 1) end, :invalid_argument},
          {fn -> BrainIndex.new(dim: nil) end, :invalid_argument},
          {fn ->
             BrainIndex.new(dim: 3, metric: :lorentz)
             |> elem(1)
             |> BrainIndex.insert(1, [0.5, 0.0, 0.0])
           end, :invalid_vector}
        ] do
      assert {:error, %Error{reason: ^reason}} = call.()
    end
  end

  test "unknown options raise", %{index: index} do
    assert_raise ArgumentError, fn -> BrainIndex.search(index, [1.0, 0.0], 1, filtr: %{}) end
    assert_raise ArgumentError, fn -> BrainIndex.new(dim: 2, metrc: :cosine) end
  end

  test "empty index searches return no hits", %{index: index} do
    assert {:ok, []} = BrainIndex.search(index, [1.0, 0.0], 3)
    assert {:ok, []} = BrainIndex.search(index, [1.0, 0.0], 3, exact: true)
  end
end
