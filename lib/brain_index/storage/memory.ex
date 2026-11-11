defmodule BrainIndex.Storage.Memory do
  @moduledoc """
  In-memory `BrainIndex.Storage` for tests. State lives in an Agent that outlives the collection,
  so "crash and reopen" can be simulated by starting a new collection with the same agent.

      {:ok, store} = BrainIndex.Storage.Memory.start()
      {BrainIndex.Collection, name: :t, dim: 3, storage: {BrainIndex.Storage.Memory, agent: store}}
  """
  @behaviour BrainIndex.Storage

  @doc "Starts an unlinked agent holding the log."
  def start, do: Agent.start(fn -> %{records: [], checkpoint: nil} end)

  @impl true
  def open(opts) do
    case Keyword.fetch(opts, :agent) do
      {:ok, agent} -> {:ok, agent}
      :error -> start()
    end
  end

  @impl true
  def load_checkpoint(agent) do
    case Agent.get(agent, & &1.checkpoint) do
      nil ->
        :none

      {bytes, lsn} ->
        with {:ok, index} <- BrainIndex.from_binary(bytes), do: {:ok, index, lsn}
    end
  end

  @impl true
  def replay(agent, after_lsn, acc, fun) do
    records = agent |> Agent.get(& &1.records) |> Enum.reverse()

    {acc, last} =
      Enum.reduce(records, {acc, after_lsn}, fn {lsn, _} = r, {acc, last} ->
        if lsn > after_lsn, do: {fun.(r, acc), lsn}, else: {acc, last}
      end)

    {:ok, acc, last, agent}
  end

  @impl true
  def append(agent, records) do
    Agent.update(agent, fn s -> %{s | records: Enum.reverse(records, s.records)} end)
    {:ok, agent}
  end

  @impl true
  def checkpoint(agent, index, lsn) do
    with {:ok, bytes} <- BrainIndex.to_binary(index) do
      Agent.update(agent, fn s ->
        %{s | checkpoint: {bytes, lsn}, records: Enum.filter(s.records, fn {l, _} -> l > lsn end)}
      end)

      {:ok, agent}
    end
  end

  @impl true
  def log_bytes(agent) do
    agent
    |> Agent.get(& &1.records)
    |> Enum.reduce(0, fn r, n -> n + :erlang.external_size(r) end)
  end

  @impl true
  def close(_agent), do: :ok
end
