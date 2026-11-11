defmodule BrainIndex.Native do
  @moduledoc false
  use Rustler, otp_app: :brain_index, crate: "brain_index_nif"

  def nif_version, do: :erlang.nif_error(:nif_not_loaded)
end
