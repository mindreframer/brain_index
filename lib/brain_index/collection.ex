defmodule BrainIndex.Collection do
  @moduledoc """
  A durable, supervised index (ADR007): every acknowledged write is in the write-ahead log.

      children = [
        {BrainIndex.Collection,
         name: :brain, dim: 768, storage: {BrainIndex.Storage.File, dir: "/var/lib/brain"}}
      ]

      :ok = BrainIndex.Collection.upsert_many(:brain, [{1, vector, %{"kind" => "kb"}}])
      {:ok, hits} = BrainIndex.Collection.search(:brain, query, 5, filter: %{"kind" => "kb"})

  Writes are serialised through the collection process. Each one is applied, appended to the log,
  made durable, and only then acknowledged. Searches read the index directly and concurrently,
  without calling the process. On start the collection recovers from the newest checkpoint plus
  the log. A checkpoint runs automatically when the log exceeds `:checkpoint_bytes`.

  Options: `:name` (required atom), `:dim` (required), `:metric`, `:m`, `:ef_construction`
  (see `BrainIndex.new/1`), `:storage` (`{module, opts}`, default in-memory), and
  `:checkpoint_bytes` (default 256 MiB).
  """
  use GenServer

  alias BrainIndex.{Error, Native}

  @type name :: atom()

  @default_checkpoint_bytes 256 * 1024 * 1024

  def child_spec(opts) do
    %{id: {__MODULE__, Keyword.fetch!(opts, :name)}, start: {__MODULE__, :start_link, [opts]}}
  end

  def start_link(opts) do
    name = Keyword.fetch!(opts, :name)
    GenServer.start_link(__MODULE__, opts, name: name)
  end

  @doc "Inserts or replaces one vector. Returns after the write is durable."
  @spec upsert(name(), BrainIndex.id(), BrainIndex.vector(), BrainIndex.meta()) ::
          :ok | {:error, Error.t()}
  def upsert(name, id, vector, meta \\ %{}), do: upsert_many(name, [{id, vector, meta}])

  @doc "Inserts or replaces many vectors atomically (one log append, one fsync)."
  @spec upsert_many(name(), [
          {BrainIndex.id(), BrainIndex.vector()}
          | {BrainIndex.id(), BrainIndex.vector(), BrainIndex.meta()}
        ]) ::
          :ok | {:error, Error.t()}
  def upsert_many(name, items) do
    items
    |> Enum.with_index()
    |> Enum.reduce_while({:ok, []}, fn {item, i}, {:ok, acc} ->
      case BrainIndex.prepare_item(item) do
        {:ok, prepared} -> {:cont, {:ok, [prepared | acc]}}
        {:error, e} -> {:halt, {:error, %{e | message: "item #{i}: #{e.message}"}}}
      end
    end)
    |> case do
      {:ok, prepared} -> GenServer.call(name, {:upsert, Enum.reverse(prepared)}, :infinity)
      error -> error
    end
  end

  @doc "Deletes `id`. Returns `{:ok, existed?}` after the write is durable."
  @spec delete(name(), BrainIndex.id()) :: {:ok, boolean()} | {:error, Error.t()}
  def delete(name, id) do
    with {:ok, id} <- BrainIndex.prepare_id(id),
         do: GenServer.call(name, {:delete, id}, :infinity)
  end

  @doc "Searches without calling the collection process. Same options as `BrainIndex.search/4`."
  @spec search(name(), BrainIndex.vector(), pos_integer(), keyword()) ::
          {:ok, [BrainIndex.hit()]} | {:error, Error.t()}
  def search(name, query, k, opts \\ []), do: BrainIndex.search(index(name), query, k, opts)

  @doc "The underlying index, for read-only use (`BrainIndex.search/4`, `BrainIndex.info/1`)."
  @spec index(name()) :: BrainIndex.t()
  def index(name), do: :persistent_term.get({__MODULE__, name})

  @doc "Forces a checkpoint: snapshot the index and drop the covered log."
  @spec checkpoint(name()) :: :ok | {:error, term()}
  def checkpoint(name), do: GenServer.call(name, :checkpoint, :infinity)

  @doc "Index info plus `:lsn` (last durable write) and `:log_bytes`."
  @spec info(name()) :: map()
  def info(name), do: GenServer.call(name, :info)

  # --- server ---------------------------------------------------------------------------------

  @impl true
  def init(opts) do
    {mod, storage_opts} = Keyword.get(opts, :storage, {BrainIndex.Storage.Memory, []})
    index_opts = Keyword.take(opts, [:dim, :metric, :m, :ef_construction])

    with {:ok, s} <- mod.open(storage_opts),
         {:ok, index, cp_lsn} <- base_index(mod, s, index_opts),
         {:ok, :ok, lsn, s} <- mod.replay(s, cp_lsn, :ok, &replay(index, &1, &2)) do
      :persistent_term.put({__MODULE__, opts[:name]}, index)

      {:ok,
       %{
         name: opts[:name],
         index: index,
         mod: mod,
         s: s,
         lsn: lsn,
         checkpoint_bytes: Keyword.get(opts, :checkpoint_bytes, @default_checkpoint_bytes)
       }}
    else
      {:error, reason} -> {:stop, reason}
    end
  end

  defp base_index(mod, s, index_opts) do
    case mod.load_checkpoint(s) do
      {:ok, index, lsn} ->
        if BrainIndex.info(index).dim == index_opts[:dim],
          do: {:ok, index, lsn},
          else: {:error, {:dimension_mismatch, BrainIndex.info(index).dim, index_opts[:dim]}}

      :none ->
        with {:ok, index} <- BrainIndex.new(index_opts), do: {:ok, index, 0}

      {:error, _} = error ->
        error
    end
  end

  # Logged ops were validated before logging, so a replay failure means corruption.
  defp replay(index, {_lsn, {:upsert, id, bin, meta}}, :ok) do
    {:ok, :ok} = Native.insert(index, id, bin, meta)
    :ok
  end

  defp replay(index, {_lsn, {:delete, id}}, :ok) do
    {:ok, _} = Native.delete(index, id)
    :ok
  end

  @impl true
  def handle_call({:upsert, items}, _from, st) do
    case Error.wrap(Native.insert_many(st.index, items)) do
      :ok -> durable(st, Enum.map(items, fn {id, bin, meta} -> {:upsert, id, bin, meta} end), :ok)
      {:error, _} = error -> {:reply, error, st}
    end
  end

  def handle_call({:delete, id}, _from, st) do
    case Error.wrap(Native.delete(st.index, id)) do
      {:ok, true} -> durable(st, [{:delete, id}], {:ok, true})
      other -> {:reply, other, st}
    end
  end

  def handle_call(:checkpoint, _from, st) do
    case st.mod.checkpoint(st.s, st.index, st.lsn) do
      {:ok, s} -> {:reply, :ok, %{st | s: s}}
      {:error, _} = error -> {:reply, error, st}
    end
  end

  def handle_call(:info, _from, st) do
    {:reply,
     Map.merge(BrainIndex.info(st.index), %{lsn: st.lsn, log_bytes: st.mod.log_bytes(st.s)}), st}
  end

  # The index already contains the ops. If they cannot be made durable, crash: the supervisor
  # restarts the collection and recovery rebuilds the index from disk without them.
  defp durable(st, ops, reply) do
    records = Enum.with_index(ops, fn op, i -> {st.lsn + 1 + i, op} end)

    case st.mod.append(st.s, records) do
      {:ok, s} ->
        st = maybe_checkpoint(%{st | s: s, lsn: st.lsn + length(ops)})
        {:reply, reply, st}

      {:error, reason} ->
        {:stop, {:storage_failed, reason},
         {:error, %Error{reason: :io, message: inspect(reason)}}, st}
    end
  end

  defp maybe_checkpoint(st) do
    if st.mod.log_bytes(st.s) > st.checkpoint_bytes do
      case st.mod.checkpoint(st.s, st.index, st.lsn) do
        {:ok, s} -> %{st | s: s}
        {:error, reason} -> exit({:checkpoint_failed, reason})
      end
    else
      st
    end
  end

  @impl true
  def terminate(_reason, st) do
    st.mod.close(st.s)
  end
end
