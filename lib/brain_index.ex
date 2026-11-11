defmodule BrainIndex do
  @moduledoc """
  Embedded vector index with exact metadata filters, implemented as a Rust NIF.

  BrainIndex stores vectors and finds the nearest ones fast. It has no server and no
  background processes, and its only dependency is Rustler. Embeddings, documents and
  the system of record belong to your application (ADR001).

      {:ok, index} = BrainIndex.new(dim: 768, metric: :cosine)

      :ok =
        BrainIndex.insert_many(index, [
          {1, embedding_1, %{"kind" => "kb", "lang" => "de"}},
          {2, embedding_2, %{"kind" => "ticket", "lang" => "en"}}
        ])

      {:ok, [{id, distance} | _]} =
        BrainIndex.search(index, query_embedding, 5, filter: %{"kind" => "kb"})

      :ok = BrainIndex.save(index, "brain.brix")
      {:ok, index} = BrainIndex.load("brain.brix")

  ## Guarantees

    * Searches run concurrently from any number of processes. Writes take an exclusive lock.
    * Filtered searches whose filter allows at most `:brute_force_threshold` vectors are exact.
    * Every function returns `{:error, %BrainIndex.Error{}}` for invalid input and never raises
      for bad data. Unknown option names raise `ArgumentError`, because that's a programming error.
    * Snapshots are written atomically. A corrupt file loads as `:corrupt_snapshot`.

  ## Metrics

    * `:cosine` - vectors are L2-normalised; distance is `1 - cos` (0..2).
    * `:lorentz` - hyperboloid points with the time component first (`x0 >= 1`); distance is
      the hyperbolic geodesic distance.
  """

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
  @type hit :: {id(), distance :: float()}
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
  Creates an empty index.

  Options:

    * `:dim` - vector dimension (required)
    * `:metric` - `:cosine` (default) or `:lorentz`
    * `:m` - HNSW graph degree, 2..256 (default 16)
    * `:ef_construction` - build beam width (default 200)
  """
  @spec new(keyword()) :: {:ok, t()} | {:error, Error.t()}
  def new(opts) do
    opts = Keyword.validate!(opts, [:dim, metric: :cosine, m: 16, ef_construction: 200])

    with {:ok, dim} <- pos_int(opts[:dim], :dim),
         {:ok, m} <- pos_int(opts[:m], :m),
         {:ok, ef_c} <- pos_int(opts[:ef_construction], :ef_construction),
         {:ok, metric} <- metric(opts[:metric]) do
      Error.wrap(Native.new(dim, metric, m, ef_c))
    end
  end

  @doc "Inserts (or replaces) the vector stored under `id`, with optional metadata."
  @spec insert(t(), id(), vector(), meta()) :: :ok | {:error, Error.t()}
  def insert(index, id, vector, meta \\ %{}) do
    with {:ok, id} <- id(id), {:ok, vector} <- vector(vector), {:ok, meta} <- meta(meta) do
      Error.wrap(Native.insert(index, id, vector, meta))
    end
  end

  @doc """
  Inserts many `{id, vector}` or `{id, vector, meta}` items under one write lock.

  All items are validated before any is inserted, so an invalid item leaves the index unchanged.
  """
  @spec insert_many(t(), [{id(), vector()} | {id(), vector(), meta()}]) ::
          :ok | {:error, Error.t()}
  def insert_many(index, items) when is_list(items) do
    items
    |> Enum.with_index()
    |> Enum.reduce_while({:ok, []}, fn {item, i}, {:ok, acc} ->
      case item(item) do
        {:ok, encoded} -> {:cont, {:ok, [encoded | acc]}}
        {:error, %Error{} = e} -> {:halt, {:error, %{e | message: "item #{i}: #{e.message}"}}}
      end
    end)
    |> case do
      {:ok, encoded} -> Error.wrap(Native.insert_many(index, Enum.reverse(encoded)))
      error -> error
    end
  end

  @doc """
  Returns up to `k` nearest vectors as `{id, distance}`, ascending by distance.

  Options:

    * `:filter` - `%{"kind" => "kb", "lang" => ["de", "en"]}` means kind = kb AND lang IN (de, en)
    * `:ef` - HNSW beam width; higher means more accurate and slower (default `max(100, 2 * k)`)
    * `:exact` - brute force over all (filtered) vectors (default `false`)
    * `:brute_force_threshold` - filtered searches allowing at most this many vectors run
      exactly (default 4096)
  """
  @spec search(t(), vector(), pos_integer(), keyword()) :: {:ok, [hit()]} | {:error, Error.t()}
  def search(index, query, k, opts \\ []) do
    opts = Keyword.validate!(opts, [:ef, filter: %{}, exact: false, brute_force_threshold: 4096])

    with {:ok, k} <- pos_int(k, :k),
         {:ok, ef} <- pos_int(opts[:ef] || max(100, 2 * k), :ef),
         {:ok, threshold} <- non_neg_int(opts[:brute_force_threshold], :brute_force_threshold),
         {:ok, filter} <- filter(opts[:filter]),
         {:ok, query} <- vector(query) do
      Error.wrap(Native.search(index, query, filter, {k, ef, opts[:exact] == true, threshold}))
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

  @doc """
  Writes a snapshot of the whole index to `path`. The file is replaced atomically.
  """
  @spec save(t(), Path.t()) :: :ok | {:error, Error.t()}
  def save(index, path), do: Error.wrap(Native.save(index, to_string(path)))

  @doc "Loads a snapshot written by `save/2`."
  @spec load(Path.t()) :: {:ok, t()} | {:error, Error.t()}
  def load(path), do: Error.wrap(Native.load(to_string(path)))

  @doc "Serialises the whole index to a binary (same format as `save/2`)."
  @spec to_binary(t()) :: {:ok, binary()} | {:error, Error.t()}
  def to_binary(index), do: Error.wrap(Native.to_binary(index))

  @doc "Restores an index from `to_binary/1` output."
  @spec from_binary(binary()) :: {:ok, t()} | {:error, Error.t()}
  def from_binary(bytes) when is_binary(bytes), do: Error.wrap(Native.from_binary(bytes))

  @doc "Encodes a list of numbers as a little-endian f32 binary (the fastest vector input)."
  @spec encode(vector()) :: binary()
  def encode(vector) when is_binary(vector), do: vector

  def encode(vector) when is_list(vector),
    do: for(x <- vector, into: <<>>, do: <<x::float-32-little>>)

  # --- validation -----------------------------------------------------------------------------

  @doc false
  # Validates and encodes `{id, vector, meta}` into the native/log form `{id, f32_binary, pairs}`.
  def prepare_item(item), do: item(item)

  @doc false
  def prepare_id(id), do: id(id)

  defp item({id, v}), do: item({id, v, %{}})

  defp item({id, v, meta}) do
    with {:ok, id} <- id(id), {:ok, v} <- vector(v), {:ok, meta} <- meta(meta) do
      {:ok, {id, v, meta}}
    end
  end

  defp item(other),
    do: invalid("expected {id, vector} or {id, vector, meta}, got: #{inspect(other)}")

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
