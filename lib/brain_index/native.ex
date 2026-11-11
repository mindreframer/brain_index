defmodule BrainIndex.Native do
  @moduledoc false
  use Rustler, otp_app: :brain_index, crate: "brain_index_nif"

  def nif_version, do: err()
  def new(_dim, _metric, _m, _ef_construction), do: err()
  def insert(_index, _id, _vector, _meta), do: err()
  def insert_many(_index, _items), do: err()
  def search(_index, _query, _filter, _opts), do: err()
  def delete(_index, _id), do: err()
  def info(_index), do: err()
  def save(_index, _path), do: err()
  def load(_path), do: err()

  defp err, do: :erlang.nif_error(:nif_not_loaded)
end
