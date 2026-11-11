defmodule BrainIndex.CollectionTest do
  use ExUnit.Case, async: true
  alias BrainIndex.{Collection, Error}
  alias BrainIndex.Storage.Memory

  # Memory backend whose appends fail while the agent holds `fail: true` (crash simulation).
  defmodule FlakyStorage do
    @behaviour BrainIndex.Storage
    defdelegate open(opts), to: Memory
    defdelegate load_checkpoint(s), to: Memory
    defdelegate replay(s, after_lsn, acc, fun), to: Memory
    defdelegate checkpoint(s, index, lsn), to: Memory
    defdelegate log_bytes(s), to: Memory
    defdelegate close(s), to: Memory

    def append(agent, records) do
      if Agent.get(agent, &Map.get(&1, :fail, false)),
        do: {:error, :disk_full},
        else: Memory.append(agent, records)
    end
  end

  setup context do
    {:ok, agent} = Memory.start()
    on_exit(fn -> Process.exit(agent, :kill) end)
    name = :"coll_#{System.unique_integer([:positive])}"
    %{agent: agent, name: name, start: fn opts -> start(name, agent, opts, context) end}
  end

  defp start(name, agent, opts, _context) do
    storage = {Keyword.get(opts, :backend, Memory), agent: agent}
    opts = Keyword.merge([name: name, dim: 2, storage: storage], Keyword.delete(opts, :backend))
    {:ok, pid} = Collection.start_link(opts)
    Process.unlink(pid)
    pid
  end

  defp stop(pid), do: GenServer.stop(pid)
  defp ids({:ok, hits}), do: Enum.map(hits, &elem(&1, 0))

  test "acknowledged writes survive a restart", %{name: name, start: start} do
    pid = start.([])

    :ok =
      Collection.upsert_many(name, [{1, [1.0, 0.0], %{kind: "a"}}, {2, [0.0, 1.0], %{kind: "b"}}])

    :ok = Collection.upsert(name, 3, [1.0, 0.1], %{kind: "a"})
    {:ok, true} = Collection.delete(name, 2)
    {:ok, false} = Collection.delete(name, 99)
    assert %{lsn: 4, size: 2} = Collection.info(name)
    stop(pid)

    start.([])
    assert ids(Collection.search(name, [1.0, 0.0], 5)) == [1, 3]
    assert ids(Collection.search(name, [1.0, 0.0], 5, filter: %{kind: "b"})) == []
    assert %{lsn: 4, size: 2} = Collection.info(name)
  end

  test "checkpoint + later writes recover", %{name: name, start: start, agent: agent} do
    pid = start.([])
    :ok = Collection.upsert_many(name, for(i <- 1..20, do: {i, [1.0, i * 1.0]}))
    :ok = Collection.checkpoint(name)
    assert Collection.info(name).log_bytes == 0
    :ok = Collection.upsert(name, 21, [1.0, 21.0])
    stop(pid)

    start.([])
    assert %{size: 21, lsn: 21} = Collection.info(name)
    assert {_, 21} = Agent.get(agent, & &1.checkpoint) |> then(fn {_, lsn} -> {nil, lsn + 1} end)
  end

  test "invalid writes are rejected and never logged", %{name: name, start: start, agent: agent} do
    start.([])

    assert {:error, %Error{reason: :dimension_mismatch}} =
             Collection.upsert(name, 1, [1.0, 0.0, 0.0])

    assert {:error, %Error{reason: :invalid_vector, message: "item 1: " <> _}} =
             Collection.upsert_many(name, [{1, [1.0, 0.0]}, {2, [:x, 1.0]}])

    assert {:error, %Error{reason: :invalid_vector}} =
             Collection.upsert_many(name, [{1, [1.0, 0.0]}, {2, [0.0, 0.0]}])

    assert Agent.get(agent, & &1.records) == []
    assert %{size: 0, lsn: 0} = Collection.info(name)
  end

  test "searches do not go through the collection process", %{name: name, start: start} do
    pid = start.([])
    :ok = Collection.upsert(name, 1, [1.0, 0.0])
    :sys.suspend(pid)
    assert ids(Collection.search(name, [1.0, 0.0], 1)) == [1]
    :sys.resume(pid)
  end

  test "auto checkpoint when the log exceeds checkpoint_bytes", %{
    name: name,
    start: start,
    agent: agent
  } do
    start.(checkpoint_bytes: 500)
    for i <- 1..20, do: :ok = Collection.upsert(name, i, [1.0, i * 1.0])
    assert {_bytes, lsn} = Agent.get(agent, & &1.checkpoint)
    assert lsn > 0
    assert Collection.info(name).log_bytes <= 500
  end

  @tag :capture_log
  test "a write that cannot be made durable crashes the collection and is not recovered",
       %{name: name, start: start, agent: agent} do
    pid = start.(backend: FlakyStorage)
    :ok = Collection.upsert(name, 1, [1.0, 0.0])
    Agent.update(agent, &Map.put(&1, :fail, true))
    ref = Process.monitor(pid)
    assert {:error, %Error{reason: :io}} = Collection.upsert(name, 2, [0.0, 1.0])
    assert_receive {:DOWN, ^ref, :process, ^pid, {:storage_failed, :disk_full}}

    Agent.update(agent, &Map.put(&1, :fail, false))
    start.(backend: FlakyStorage)
    assert ids(Collection.search(name, [0.0, 1.0], 5)) == [1]
  end

  @tag :capture_log
  test "refuses to start when the checkpoint dimension differs", %{
    name: name,
    start: start,
    agent: agent
  } do
    pid = start.([])
    :ok = Collection.upsert(name, 1, [1.0, 0.0])
    :ok = Collection.checkpoint(name)
    stop(pid)
    Process.flag(:trap_exit, true)

    assert {:error, {:dimension_mismatch, 2, 3}} =
             Collection.start_link(name: name, dim: 3, storage: {Memory, agent: agent})
  end
end
