defmodule BrainIndex do
  @moduledoc """
  Embedded vector index with exact metadata filters, implemented as a Rust NIF.

  The index lives on disk in LMDB (ADR009). Every write is one transaction that writes only the
  changed pages, durable on return. Reopening is instant and the data doesn't need to fit in RAM.
  Each id can carry an opaque payload (ADR010).

      {:ok, index} = BrainIndex.open("/var/lib/brain", dim: 768)

      :ok =
        BrainIndex.insert_many(index, [
          {1, embedding_1, %{"kind" => "kb", "lang" => "de"}, chunk_text_1},
          {2, embedding_2, %{"kind" => "ticket", "lang" => "en"}, chunk_text_2}
        ])

      {:ok, [{1, distance, chunk_text_1} | _]} =
        BrainIndex.search(index, query_embedding, 5, filter: %{"kind" => "kb"}, payload: true)

  `new/1` creates a temporary index (deleted when garbage-collected), with the same behaviour
  and no durability. It's meant for tests and scratch work.

  ## Guarantees

    * Searches run concurrently from any number of processes. Writes take an exclusive lock.
    * Filtered searches whose filter allows at most `:brute_force_threshold` vectors are exact.
    * Every function returns `{:error, %BrainIndex.Error{}}` for invalid input and never raises
      for bad data. Unknown option names raise `ArgumentError`, because that's a programming error.
    * Writes are crash-safe: after a crash the index is exactly the last committed state, with
      nothing to replay. Open one directory once per OS process.

  ## Metrics

    * `:cosine` - vectors are L2-normalised; distance is `1 - cos` (0..2).
    * `:lorentz` - hyperboloid points with the time component first (`x0 >= 1`); distance is
      the hyperbolic geodesic distance.
  """

  import Bitwise, only: [<<<: 2]
  alias BrainIndex.{Error, Native}

  @opaque t :: reference()
  @typedoc "Application-chosen unsigned 64-bit id. Inserting an existing id replaces it."
  @type id :: non_neg_integer()
  @typedoc "List of numbers, or a binary of little-endian f32 (fastest)."
  @type vector :: [number()] | binary()
  @typedoc "Exact-match metadata. Keys and values are converted with `to_string/1`."
  @type meta :: %{optional(String.t() | atom()) => String.t() | atom() | number()}
  @typedoc "AND across keys, OR across a key's list of values."
  @type filter :: %{optional(String.t() | atom()) => term() | [term()]}
  @type hit :: {id(), distance :: float()} | {id(), distance :: float(), binary() | nil}
  @type item ::
          {id(), vector()} | {id(), vector(), meta()} | {id(), vector(), meta(), binary() | nil}
  @type info :: %{
          dim: pos_integer(),
          metric: :cosine | :lorentz,
          size: non_neg_integer(),
          deleted: non_neg_integer(),
          m: pos_integer(),
          ef_construction: pos_integer()
        }

  @max_u64 0xFFFF_FFFF_FFFF_FFFF

  @doc """
  Opens (or creates) a durable index stored in directory `dir`.

  Options: `:dim` (required), `:metric` (`:cosine` default, or `:lorentz`), `:m` (graph degree,
  default 16), `:ef_construction` (default 200), `:map_size` (maximum file size, default 1 TiB,
  sparse), `:sync` (fsync on commit, default `true`). Reopening an existing index requires the same
  `:dim` and `:metric`.
  """
  @spec open(Path.t(), keyword()) :: {:ok, t()} | {:error, Error.t()}
  def open(dir, opts) do
    with {:ok, config} <- config(opts), do: Error.wrap(Native.open(to_string(dir), config))
  end

  @doc "Creates a temporary index (same options as `open/2`), deleted when garbage-collected."
  @spec new(keyword()) :: {:ok, t()} | {:error, Error.t()}
  def new(opts) do
    with {:ok, config} <- config(opts), do: Error.wrap(Native.temp(config))
  end

  defp config(opts) do
    opts =
      Keyword.validate!(opts, [
        :dim,
        metric: :cosine,
        m: 16,
        ef_construction: 200,
        map_size: 1 <<< 40,
        sync: true
      ])

    with {:ok, dim} <- pos_int(opts[:dim], :dim),
         {:ok, m} <- pos_int(opts[:m], :m),
         {:ok, ef_c} <- pos_int(opts[:ef_construction], :ef_construction),
         {:ok, map_size} <- pos_int(opts[:map_size], :map_size),
         {:ok, metric} <- metric(opts[:metric]) do
      {:ok, {dim, metric, m, ef_c, map_size, opts[:sync] == true}}
    end
  end

  @doc "Inserts (or replaces) the vector stored under `id`, with optional metadata and payload."
  @spec insert(t(), id(), vector(), meta(), binary() | nil) :: :ok | {:error, Error.t()}
  def insert(index, id, vector, meta \\ %{}, payload \\ nil),
    do: insert_many(index, [{id, vector, meta, payload}])

  @doc """
  Inserts many items in one transaction (one commit, one fsync).

  Items are `{id, vector}`, `{id, vector, meta}` or `{id, vector, meta, payload}`. All items are
  validated before anything is written, so an invalid item leaves the index unchanged.
  """
  @spec insert_many(t(), [item()]) :: :ok | {:error, Error.t()}
  def insert_many(index, items) when is_list(items) do
    with {:ok, encoded} <- prepare_items(items), do: write_prepared(index, encoded)
  end

  @doc false
  # Validates and encodes items in the caller's process; used by `BrainIndex.Writer`.
  def prepare_items(items) do
    items
    |> Enum.with_index()
    |> Enum.reduce_while({:ok, []}, fn {item, i}, {:ok, acc} ->
      case item(item) do
        {:ok, encoded} -> {:cont, {:ok, [encoded | acc]}}
        {:error, %Error{} = e} -> {:halt, {:error, %{e | message: "item #{i}: #{e.message}"}}}
      end
    end)
    |> case do
      {:ok, encoded} -> {:ok, Enum.reverse(encoded)}
      error -> error
    end
  end

  @doc false
  def write_prepared(index, encoded), do: Error.wrap(Native.insert_many(index, encoded))

  @doc """
  Returns up to `k` nearest vectors as `{id, distance}`, ascending by distance.

  Options:

    * `:filter` - `%{"kind" => "kb", "lang" => ["de", "en"]}` means kind = kb AND lang IN (de, en)
    * `:ef` - HNSW beam width; higher means more accurate and slower (default `max(100, 2 * k)`)
    * `:exact` - brute force over all (filtered) vectors (default `false`)
    * `:brute_force_threshold` - filtered searches allowing at most this many vectors run
      exactly (default 4096)
    * `:payload` - return `{id, distance, payload}` (payload is `nil` when none was stored)
  """
  @spec search(t(), vector(), pos_integer(), keyword()) :: {:ok, [hit()]} | {:error, Error.t()}
  def search(index, query, k, opts \\ []) do
    opts =
      Keyword.validate!(opts, [
        :ef,
        filter: %{},
        exact: false,
        brute_force_threshold: 4096,
        payload: false
      ])

    with {:ok, k} <- pos_int(k, :k),
         {:ok, ef} <- pos_int(opts[:ef] || max(100, 2 * k), :ef),
         {:ok, threshold} <- non_neg_int(opts[:brute_force_threshold], :brute_force_threshold),
         {:ok, filter} <- filter(opts[:filter]),
         {:ok, query} <- vector(query) do
      Error.wrap(
        Native.search(
          index,
          query,
          filter,
          {k, ef, opts[:exact] == true, threshold, opts[:payload] == true}
        )
      )
    end
  end

  @doc "Deletes `id`. Returns `{:ok, true}` if it existed."
  @spec delete(t(), id()) :: {:ok, boolean()} | {:error, Error.t()}
  def delete(index, id) do
    with {:ok, id} <- id(id), do: Error.wrap(Native.delete(index, id))
  end

  @doc "Number of live (non-deleted) vectors."
  @spec size(t()) :: non_neg_integer()
  def size(index), do: info(index).size

  @doc "Index configuration and counters."
  @spec info(t()) :: info()
  def info(index) do
    {:ok, {dim, metric, size, deleted, m, ef_c}} = Native.info(index)

    %{
      dim: dim,
      metric: String.to_existing_atom(metric),
      size: size,
      deleted: deleted,
      m: m,
      ef_construction: ef_c
    }
  end

  @doc "The payload stored under `id`, or `nil`."
  @spec get_payload(t(), id()) :: {:ok, binary() | nil} | {:error, Error.t()}
  def get_payload(index, id) do
    with {:ok, id} <- id(id), do: Error.wrap(Native.get_payload(index, id))
  end

  @doc """
  Writes a consistent, compacted copy of the index into directory `dir` while the index stays
  online. The copy opens with `open/2`.
  """
  @spec backup(t(), Path.t()) :: :ok | {:error, Error.t()}
  def backup(index, dir), do: Error.wrap(Native.backup(index, to_string(dir)))

  @doc """
  Rebuilds the live vectors, metadata and payloads into a fresh index in the empty directory
  `dir`. Tombstones are dropped and the graph is rebuilt without them. Runs on a snapshot, so the
  source stays online, but writes made during the run are not copied: pause writers (or replay
  them) before switching over. Returns the number of vectors copied.

  Use it when `info/1` reports many `deleted` nodes. Options: `batch:` (default 1000).
  """
  @spec compact(t(), Path.t(), keyword()) :: {:ok, non_neg_integer()} | {:error, Error.t()}
  def compact(index, dir, opts \\ []) do
    batch = Keyword.get(opts, :batch, 1000)
    Error.wrap(Native.compact(index, to_string(dir), batch))
  end

  @doc "Encodes a list of numbers as a little-endian f32 binary (the fastest vector input)."
  @spec encode(vector()) :: binary()
  def encode(vector) when is_binary(vector), do: vector

  def encode(vector) when is_list(vector),
    do: for(x <- vector, into: <<>>, do: <<x::float-32-little>>)

  # --- validation -----------------------------------------------------------------------------

  defp item({id, v}), do: item({id, v, %{}, nil})
  defp item({id, v, meta}), do: item({id, v, meta, nil})

  defp item({id, v, meta, payload}) do
    with {:ok, id} <- id(id),
         {:ok, v} <- vector(v),
         {:ok, meta} <- meta(meta),
         {:ok, payload} <- payload(payload) do
      {:ok, {id, v, meta, payload}}
    end
  end

  defp item(other),
    do: invalid("expected {id, vector} or {id, vector, meta}, got: #{inspect(other)}")

  defp payload(p) when is_binary(p) or is_nil(p), do: {:ok, p}
  defp payload(p), do: invalid("payload must be a binary or nil, got: #{inspect(p)}")

  defp id(id) when is_integer(id) and id >= 0 and id <= @max_u64, do: {:ok, id}
  defp id(id), do: invalid("id must be an unsigned 64-bit integer, got: #{inspect(id)}")

  defp vector(v) when is_binary(v), do: {:ok, v}

  defp vector(v) when is_list(v) do
    if Enum.all?(v, &is_number/1),
      do: {:ok, encode(v)},
      else: {:error, err(:invalid_vector, "vector must contain numbers")}
  end

  defp vector(v),
    do:
      {:error, err(:invalid_vector, "expected list of numbers or f32 binary, got: #{inspect(v)}")}

  defp meta(meta) when is_map(meta),
    do: {:ok, Enum.map(meta, fn {k, v} -> {to_string(k), to_string(v)} end)}

  defp meta(meta), do: invalid("metadata must be a map, got: #{inspect(meta)}")

  defp filter(f) when is_map(f),
    do:
      {:ok,
       Enum.map(f, fn {k, v} -> {to_string(k), v |> List.wrap() |> Enum.map(&to_string/1)} end)}

  defp filter(f), do: invalid("filter must be a map, got: #{inspect(f)}")

  defp metric(m) when m in [:cosine, :lorentz], do: {:ok, Atom.to_string(m)}
  defp metric(m), do: invalid("metric must be :cosine or :lorentz, got: #{inspect(m)}")

  defp pos_int(n, _name) when is_integer(n) and n > 0, do: {:ok, n}
  defp pos_int(n, name), do: invalid("#{name} must be a positive integer, got: #{inspect(n)}")

  defp non_neg_int(n, _name) when is_integer(n) and n >= 0, do: {:ok, n}

  defp non_neg_int(n, name),
    do: invalid("#{name} must be a non-negative integer, got: #{inspect(n)}")

  defp invalid(message), do: {:error, err(:invalid_argument, message)}
  defp err(reason, message), do: %Error{reason: reason, message: message}
end
