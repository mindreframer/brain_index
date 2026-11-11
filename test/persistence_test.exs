defmodule BrainIndex.PersistenceTest do
  use ExUnit.Case, async: true
  alias BrainIndex.{Error, Fixtures}

  @moduletag :tmp_dir

  setup_all do
    {:ok, index} = BrainIndex.new(dim: 129, metric: :lorentz)

    Enum.zip(Fixtures.chunks(), Fixtures.vectors(:v5, :chunks))
    |> Enum.each(fn {c, v} -> :ok = BrainIndex.insert(index, c.id, v, %{"kind" => c.kind}) end)

    %{index: index, queries: Enum.take(Fixtures.vectors(:v5, :queries), 20)}
  end

  test "save/load roundtrip gives identical results", %{index: index, queries: qs, tmp_dir: dir} do
    path = Path.join(dir, "brain.brix")
    assert :ok = BrainIndex.save(index, path)
    assert {:ok, loaded} = BrainIndex.load(path)
    assert BrainIndex.size(loaded) == 543

    for q <- qs, filter <- [%{}, %{"kind" => "kb"}] do
      assert BrainIndex.search(index, q, 10, filter: filter) ==
               BrainIndex.search(loaded, q, 10, filter: filter)
    end
  end

  test "delete and upsert" do
    {:ok, index} = BrainIndex.new(dim: 2)
    :ok = BrainIndex.insert(index, 1, [1.0, 0.0], %{kind: "a"})
    :ok = BrainIndex.insert(index, 2, [0.0, 1.0], %{kind: "b"})
    assert {:ok, true} = BrainIndex.delete(index, 1)
    assert {:ok, false} = BrainIndex.delete(index, 1)
    assert {:ok, [{2, _}]} = BrainIndex.search(index, [1.0, 0.0], 5)
    :ok = BrainIndex.insert(index, 2, [1.0, 0.1], %{kind: "a"})
    assert {:ok, [{2, _}]} = BrainIndex.search(index, [1.0, 0.0], 5, filter: %{kind: "a"})
    assert {:ok, []} = BrainIndex.search(index, [1.0, 0.0], 5, filter: %{kind: "b"})
    assert BrainIndex.size(index) == 1
  end

  test "corrupt and missing files return tagged errors", %{index: index, tmp_dir: dir} do
    path = Path.join(dir, "c.brix")
    :ok = BrainIndex.save(index, path)
    bytes = File.read!(path)
    <<head::binary-size(1000), b, rest::binary>> = bytes
    File.write!(path, <<head::binary, Bitwise.bxor(b, 1), rest::binary>>)
    assert {:error, %Error{reason: :corrupt_snapshot}} = BrainIndex.load(path)
    File.write!(path, binary_part(bytes, 0, 500))
    assert {:error, %Error{reason: :corrupt_snapshot}} = BrainIndex.load(path)
    assert {:error, %Error{reason: :io}} = BrainIndex.load(Path.join(dir, "missing.brix"))
  end
end
