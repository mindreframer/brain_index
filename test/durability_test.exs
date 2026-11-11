defmodule BrainIndex.DurabilityTest do
  @moduledoc "Crash matrix for Collection + Storage.File (ROADMAP002 quality bar)."
  use ExUnit.Case, async: true
  alias BrainIndex.Collection

  @moduletag :tmp_dir

  defp start(dir, opts \\ []) do
    name = :"dur_#{System.unique_integer([:positive])}"
    storage = {BrainIndex.Storage.File, [dir: dir] ++ Keyword.take(opts, [:segment_bytes])}
    opts = [name: name, dim: 4, storage: storage] ++ Keyword.take(opts, [:checkpoint_bytes])
    {:ok, pid} = Collection.start_link(opts)
    Process.unlink(pid)
    {name, pid}
  end

  defp kill(pid) do
    ref = Process.monitor(pid)
    Process.exit(pid, :kill)
    assert_receive {:DOWN, ^ref, _, _, _}
  end

  defp vec(i), do: [1.0, :math.sin(i), :math.cos(i), rem(i, 5) * 1.0]

  defp all_ids(name),
    do:
      Collection.search(name, vec(0), 1000, exact: true)
      |> elem(1)
      |> Enum.map(&elem(&1, 0))
      |> Enum.sort()

  test "brutal kill after acknowledged writes loses nothing", %{tmp_dir: dir} do
    {name, pid} = start(dir)
    for i <- 1..50, do: :ok = Collection.upsert(name, i, vec(i), %{"p" => rem(i, 2)})
    {:ok, true} = Collection.delete(name, 7)
    kill(pid)

    {name, _} = start(dir)
    assert all_ids(name) == Enum.to_list(1..50) -- [7]
    assert {:ok, hits} = Collection.search(name, vec(3), 3, filter: %{"p" => 1})
    assert Enum.all?(hits, fn {id, _} -> rem(id, 2) == 1 end)
  end

  test "crash after checkpoint write but before segment cleanup recovers", %{tmp_dir: dir} do
    {name, pid} = start(dir)
    :ok = Collection.upsert_many(name, for(i <- 1..30, do: {i, vec(i)}))

    saved =
      for f <- File.ls!(dir),
          String.starts_with?(f, "wal-"),
          do: {f, File.read!(Path.join(dir, f))}

    :ok = Collection.checkpoint(name)
    :ok = Collection.upsert(name, 31, vec(31))
    kill(pid)
    # Simulate the crash: the old (covered) segments were never deleted.
    for {f, bin} <- saved, do: File.write!(Path.join(dir, f), bin)

    {name, _} = start(dir)
    assert all_ids(name) == Enum.to_list(1..31)
    assert Collection.info(name).lsn == 31
  end

  test "random ops with rotation, auto checkpoints and kills match a reference index", %{
    tmp_dir: dir
  } do
    :rand.seed(:exsss, {7, 7, 7})
    {:ok, ref} = BrainIndex.new(dim: 4)
    opts = [segment_bytes: 2_000, checkpoint_bytes: 6_000]

    {name, pid} =
      Enum.reduce(1..400, start(dir, opts), fn step, {name, pid} ->
        id = :rand.uniform(120)

        if :rand.uniform() < 0.25 do
          {:ok, existed} = BrainIndex.delete(ref, id)
          assert {:ok, ^existed} = Collection.delete(name, id)
        else
          :ok = BrainIndex.insert(ref, id, vec(step), %{"s" => rem(step, 3)})
          :ok = Collection.upsert(name, id, vec(step), %{"s" => rem(step, 3)})
        end

        if rem(step, 97) == 0 do
          kill(pid)
          start(dir, opts)
        else
          {name, pid}
        end
      end)

    assert Collection.info(name).lsn > 300
    kill(pid)
    {name, _} = start(dir, opts)

    expected =
      BrainIndex.search(ref, vec(0), 1000, exact: true)
      |> elem(1)
      |> Enum.map(&elem(&1, 0))
      |> Enum.sort()

    assert all_ids(name) == expected
    assert Enum.count(File.ls!(dir), &String.starts_with?(&1, "checkpoint-")) == 1

    for f <- [%{"s" => "1"}, %{}] do
      {:ok, got} = Collection.search(name, vec(5), 10, filter: f, exact: true)
      {:ok, want} = BrainIndex.search(ref, vec(5), 10, filter: f, exact: true)
      assert Enum.map(got, &elem(&1, 0)) == Enum.map(want, &elem(&1, 0))
      for {{_, a}, {_, b}} <- Enum.zip(got, want), do: assert_in_delta(a, b, 1.0e-6)
    end
  end
end
