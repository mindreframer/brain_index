defmodule BrainIndexTest do
  use ExUnit.Case, async: true
  alias BrainIndex.Error

  setup do
    {:ok, index} = BrainIndex.new(dim: 3, metric: :cosine)
    :ok = BrainIndex.insert(index, 1, [1.0, 0.0, 0.0])
    :ok = BrainIndex.insert(index, 2, [0.0, 1.0, 0.0])
    %{index: index}
  end

  test "search returns nearest first", %{index: index} do
    assert {:ok, [{1, d1}, {2, d2}]} = BrainIndex.search(index, [0.9, 0.1, 0.0], 5)
    assert d1 < d2
    assert BrainIndex.size(index) == 2
  end

  test "accepts f32 binaries", %{index: index} do
    q = BrainIndex.encode([0.0, 1.0, 0.0])
    assert {:ok, [{2, _} | _]} = BrainIndex.search(index, q, 1)
  end

  test "errors are tagged, never raised", %{index: index} do
    assert {:error, %Error{reason: :dimension_mismatch}} = BrainIndex.insert(index, 3, [1.0])

    assert {:error, %Error{reason: :invalid_vector}} =
             BrainIndex.insert(index, 3, [0.0, 0.0, 0.0])

    assert {:error, %Error{reason: :invalid_vector}} = BrainIndex.search(index, <<1, 2, 3>>, 1)
    assert {:error, %Error{reason: :invalid_argument}} = BrainIndex.new(dim: 0)
    assert {:error, %Error{reason: :invalid_argument}} = BrainIndex.new(dim: 3, metric: :l2)
  end
end
