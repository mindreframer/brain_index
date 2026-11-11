defmodule BrainIndex.Native do
  @moduledoc false
  use Rustler, otp_app: :brain_index, crate: "brain_index_nif"

  def nif_version, do: err()
  def open(_dir, _config), do: err()
  def temp(_config), do: err()
  def insert_many(_index, _items), do: err()
  def search(_index, _query, _filter, _opts), do: err()
  def delete(_index, _id), do: err()
  def get_payload(_index, _id), do: err()
  def info(_index), do: err()
  def backup(_index, _dir), do: err()

  defp err, do: :erlang.nif_error(:nif_not_loaded)
end
