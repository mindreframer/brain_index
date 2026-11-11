defmodule BrainIndex do
  @moduledoc """
  Embedded vector index with exact metadata filters, implemented as a Rust NIF.
  """

  alias BrainIndex.{Error, Native}

  @opaque t :: reference()
  @type id :: non_neg_integer()
  @type vector :: [number()] | binary()
  @type hit :: {id(), float()}
  @type meta :: %{optional(String.t() | atom()) => String.t() | atom()}

  @doc """
  Creates an empty index.

  Options: `:dim` (required), `:metric` (`:cosine` | `:lorentz`, default `:cosine`),
  `:m` (graph degree, default 16), `:ef_construction` (build beam width, default 200).
  """
  @spec new(keyword()) :: {:ok, t()} | {:error, Error.t()}
  def new(opts) do
    dim = Keyword.fetch!(opts, :dim)
    metric = Keyword.get(opts, :metric, :cosine)
    m = Keyword.get(opts, :m, 16)
    ef_construction = Keyword.get(opts, :ef_construction, 200)
    Error.wrap(Native.new(dim, Atom.to_string(metric), m, ef_construction))
  end

  @doc """
  Inserts a vector under `id` with optional metadata (string keys and values, exact-match filterable).
  """
  @spec insert(t(), id(), vector(), meta()) :: :ok | {:error, Error.t()}
  def insert(index, id, vector, meta \\ %{}) do
    Error.wrap(Native.insert(index, id, encode(vector), encode_meta(meta)))
  end

  @doc "Number of live vectors."
  @spec size(t()) :: non_neg_integer()
  def size(index) do
    {:ok, n} = Native.size(index)
    n
  end

  @doc """
  Returns the `k` nearest vectors as `{id, distance}`, ascending by distance.

  Options:

    * `:filter` - `%{"kind" => "kb", "lang" => ["de", "en"]}` means kind = kb AND lang IN (de, en)
    * `:ef` - HNSW search beam width, default `max(100, 2 * k)`
    * `:exact` - brute force instead of HNSW, default `false`
    * `:brute_force_threshold` - filtered searches allowing at most this many vectors run
      exactly, default 4096
  """
  @spec search(t(), vector(), pos_integer(), keyword()) :: {:ok, [hit()]} | {:error, Error.t()}
  def search(index, query, k, opts \\ []) do
    filter = opts |> Keyword.get(:filter, %{}) |> encode_filter()
    ef = Keyword.get(opts, :ef, max(100, 2 * k))
    exact = Keyword.get(opts, :exact, false)
    threshold = Keyword.get(opts, :brute_force_threshold, 4096)
    Error.wrap(Native.search(index, encode(query), filter, {k, ef, exact, threshold}))
  end

  defp encode_meta(meta), do: Enum.map(meta, fn {k, v} -> {to_string(k), to_string(v)} end)

  defp encode_filter(filter) do
    Enum.map(filter, fn {k, v} -> {to_string(k), v |> List.wrap() |> Enum.map(&to_string/1)} end)
  end

  @doc false
  def encode(vector) when is_binary(vector), do: vector

  def encode(vector) when is_list(vector),
    do: for(x <- vector, into: <<>>, do: <<x::float-32-little>>)
end
