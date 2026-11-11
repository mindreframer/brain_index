defmodule BrainIndex.ShipperTest do
  use ExUnit.Case, async: true
  alias BrainIndex.Shipper

  @moduletag :tmp_dir

  defp vec(i), do: [1.0, :math.sin(i), :math.cos(i), rem(i, 5) * 1.0]

  test "ship to a folder store, prune, restore the latest", %{tmp_dir: dir} do
    {:ok, index} = BrainIndex.open(Path.join(dir, "live"), dim: 4)
    :ok = BrainIndex.insert_many(index, for(i <- 1..30, do: {i, vec(i), %{}, "c#{i}"}))

    bucket = Path.join(dir, "bucket")
    File.mkdir_p!(bucket)
    {:ok, store} = ObjectStoreX.new(:local, path: bucket)

    {:ok, k1} = Shipper.ship(index, store, "brain")
    :ok = BrainIndex.insert(index, 31, vec(31), %{}, "c31")
    {:ok, k2} = Shipper.ship(index, store, "brain")
    {:ok, k3} = Shipper.ship(index, store, "brain")

    assert {:ok, [^k1, ^k2, ^k3]} = Shipper.list(store, "brain")
    assert {:ok, 1} = Shipper.prune(store, "brain", keep: 2)
    assert {:ok, ^k3} = Shipper.latest(store, "brain")

    restored = Path.join(dir, "restored")
    :ok = Shipper.restore(store, k3, restored)
    {:ok, copy} = BrainIndex.open(restored, dim: 4)
    assert BrainIndex.size(copy) == 31
    assert {:ok, "c31"} = BrainIndex.get_payload(copy, 31)

    assert {:error, %BrainIndex.Error{reason: :invalid_argument}} =
             Shipper.restore(store, k3, restored)
  end

  test "memory store and empty prefix" do
    {:ok, store} = ObjectStoreX.new(:memory)
    assert {:ok, nil} = Shipper.latest(store, "none")
    {:ok, index} = BrainIndex.new(dim: 4)
    :ok = BrainIndex.insert(index, 1, vec(1))
    assert {:ok, "x/" <> _} = Shipper.ship(index, store, "x")
  end
end
