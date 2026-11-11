defmodule BrainIndex.Error do
  @moduledoc """
  Stable error returned by every `BrainIndex` function (see ADR006).

  Reasons: `:invalid_argument`, `:dimension_mismatch`, `:invalid_vector`, `:io`,
  `:corrupt_snapshot`, `:internal`.
  """
  defexception [:reason, :message]

  @type t :: %__MODULE__{reason: atom(), message: String.t()}

  @doc false
  def wrap({:ok, :ok}), do: :ok
  def wrap({:ok, value}), do: {:ok, value}

  def wrap({:error, {reason, message}}),
    do: {:error, %__MODULE__{reason: reason, message: message}}
end
