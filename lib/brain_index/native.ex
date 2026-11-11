defmodule BrainIndex.Native do
  @moduledoc false
  use Rustler, otp_app: :brain_index, crate: "brain_index_nif"

  def nif_version, do: err()
  def new(_dim, _metric, _m, _ef_construction), do: err()
  def insert(_index, _id, _vector, _meta), do: err()
  def size(_index), do: err()
  def search(_index, _query, _filter, _opts), do: err()

  defp err, do: :erlang.nif_error(:nif_not_loaded)
end
