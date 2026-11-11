defmodule BrainIndex.WriterTest do
  use ExUnit.Case, async: true
  alias BrainIndex.{Error, Writer}

  setup do
    {:ok, index} = BrainIndex.new(dim: 3)

    writer =
      start_supervised!({Writer, name: :"w#{System.unique_integer([:positive])}", index: index})

    %{index: index, writer: writer}
  end

  test "concurrent inserts share commits and are all durable on return", %{
    index: index,
    writer: w
  } do
    1..100
    |> Task.async_stream(
      fn i -> Writer.insert(w, i, [1.0, i * 1.0, 0.5], %{"k" => "v"}, "p#{i}") end,
      max_concurrency: 100
    )
    |> Enum.each(fn {:ok, reply} -> assert reply == :ok end)

    assert BrainIndex.size(index) == 100
    assert %{items: 100, commits: commits} = Writer.stats(w)
    assert commits < 20
    assert {:ok, "p42"} = BrainIndex.get_payload(index, 42)
  end

  test "an invalid item fails only its own call", %{index: index, writer: w} do
    task = Task.async(fn -> Writer.insert(w, 1, [1.0, 0.0, 0.0]) end)
    assert {:error, %Error{reason: :invalid_vector}} = Writer.insert(w, 2, [0.0, 0.0, 0.0])
    assert :ok = Task.await(task)
    assert BrainIndex.size(index) == 1
  end

  test "delete waits for queued inserts", %{index: index, writer: w} do
    task = Task.async(fn -> Writer.insert(w, 7, [1.0, 0.0, 0.0]) end)
    Process.sleep(1)
    assert {:ok, true} = Writer.delete(w, 7)
    assert :ok = Task.await(task)
    assert BrainIndex.size(index) == 0
  end
end
