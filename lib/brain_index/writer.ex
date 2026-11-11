defmodule BrainIndex.Writer do
  @moduledoc """
  Group commit for many concurrent writers (ADR009).

  Every commit fsyncs and copies the B-tree pages it touched. One lone HNSW insert costs about
  0.5 MB of SSD writes, and a batch of 1000 costs about 3 KB per item. The writer merges
  concurrent `insert/5` calls that arrive within `:max_delay_ms` into one transaction. Each caller
  is answered only after its data is durable.

      children = [{BrainIndex.Writer, name: :brain_writer, index: index}]
      :ok = BrainIndex.Writer.insert(:brain_writer, id, vector, %{"kind" => "kb"}, text)

  Options: `:name` (required), `:index` (required), `:max_delay_ms` (default 5), `:max_batch`
  (default 1000). Items are validated in the calling process, so an invalid item fails only its
  own call (a
  merged commit that fails is retried per caller).
  """
  use GenServer

  alias BrainIndex.Error

  def start_link(opts),
    do: GenServer.start_link(__MODULE__, opts, name: Keyword.fetch!(opts, :name))

  def child_spec(opts),
    do: %{id: {__MODULE__, opts[:name]}, start: {__MODULE__, :start_link, [opts]}}

  @doc "Durably inserts or replaces one item, sharing the commit with concurrent callers."
  @spec insert(
          GenServer.server(),
          BrainIndex.id(),
          BrainIndex.vector(),
          BrainIndex.meta(),
          binary() | nil
        ) ::
          :ok | {:error, Error.t()}
  def insert(writer, id, vector, meta \\ %{}, payload \\ nil),
    do: insert_many(writer, [{id, vector, meta, payload}])

  @doc "Durably inserts a list of items (see `BrainIndex.insert_many/2`) in a shared commit."
  @spec insert_many(GenServer.server(), [BrainIndex.item()]) :: :ok | {:error, Error.t()}
  def insert_many(writer, items) do
    with {:ok, encoded} <- BrainIndex.prepare_items(items),
         do: GenServer.call(writer, {:insert, encoded}, :infinity)
  end

  @doc "Deletes `id` after all queued inserts are committed (preserves ordering)."
  @spec delete(GenServer.server(), BrainIndex.id()) :: {:ok, boolean()} | {:error, Error.t()}
  def delete(writer, id), do: GenServer.call(writer, {:delete, id}, :infinity)

  @doc "Counters: `%{commits: n, items: n}`."
  def stats(writer), do: GenServer.call(writer, :stats)

  @impl true
  def init(opts) do
    {:ok,
     %{
       index: Keyword.fetch!(opts, :index),
       max_delay: Keyword.get(opts, :max_delay_ms, 5),
       max_batch: Keyword.get(opts, :max_batch, 1000),
       queue: [],
       queued: 0,
       timer: nil,
       commits: 0,
       items: 0
     }}
  end

  @impl true
  def handle_call({:insert, encoded}, from, st) do
    st = %{st | queue: [{from, encoded} | st.queue], queued: st.queued + length(encoded)}

    cond do
      st.queued >= st.max_batch ->
        {:noreply, flush(st)}

      st.timer == nil ->
        {:noreply, %{st | timer: Process.send_after(self(), :flush, st.max_delay)}}

      true ->
        {:noreply, st}
    end
  end

  def handle_call({:delete, id}, _from, st) do
    st = flush(st)
    {:reply, BrainIndex.delete(st.index, id), st}
  end

  def handle_call(:stats, _from, st), do: {:reply, %{commits: st.commits, items: st.items}, st}

  @impl true
  def handle_info(:flush, st), do: {:noreply, flush(%{st | timer: nil})}

  defp flush(%{queue: []} = st), do: st

  defp flush(st) do
    if st.timer, do: Process.cancel_timer(st.timer)
    batch = Enum.reverse(st.queue)

    commits =
      case BrainIndex.write_prepared(st.index, Enum.flat_map(batch, &elem(&1, 1))) do
        :ok ->
          Enum.each(batch, fn {from, _} -> GenServer.reply(from, :ok) end)
          1

        {:error, _} ->
          # Some item failed native validation (e.g. a zero vector). The failed transaction wrote
          # nothing; commit each caller separately so only the offending call fails.
          Enum.each(batch, fn {from, items} ->
            GenServer.reply(from, BrainIndex.write_prepared(st.index, items))
          end)

          length(batch)
      end

    %{
      st
      | queue: [],
        queued: 0,
        timer: nil,
        commits: st.commits + commits,
        items: st.items + st.queued
    }
  end
end
