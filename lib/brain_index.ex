defmodule BrainIndex do
  @moduledoc """
  Embedded vector index with exact metadata filters, implemented as a Rust NIF.
  """

  alias BrainIndex.{Error, Native}

  @opaque t :: reference()
  @type id :: non_neg_integer()
  @type vector :: [number()] | binary()
  @type hit :: {id(), float()}

  @doc "Creates an empty index. Options: `:dim` (required), `:metric` (`:cosine` | `:lorentz`)."
  @spec new(keyword()) :: {:ok, t()} | {:error, Error.t()}
  def new(opts) do
    dim = Keyword.fetch!(opts, :dim)
    metric = Keyword.get(opts, :metric, :cosine)
    Error.wrap(Native.new(dim, Atom.to_string(metric)))
  end

  @doc "Inserts a vector under `id`."
  @spec insert(t(), id(), vector()) :: :ok | {:error, Error.t()}
  def insert(index, id, vector), do: Error.wrap(Native.insert(index, id, encode(vector)))

  @doc "Number of live vectors."
  @spec size(t()) :: non_neg_integer()
  def size(index) do
    {:ok, n} = Native.size(index)
    n
  end

  @doc "Returns the `k` nearest vectors as `{id, distance}`, ascending by distance."
  @spec search(t(), vector(), pos_integer(), keyword()) :: {:ok, [hit()]} | {:error, Error.t()}
  def search(index, query, k, _opts \\ []) do
    Error.wrap(Native.exact_search(index, encode(query), k))
  end

  @doc false
  def encode(vector) when is_binary(vector), do: vector

  def encode(vector) when is_list(vector),
    do: for(x <- vector, into: <<>>, do: <<x::float-32-little>>)
end
