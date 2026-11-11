defmodule BrainIndex.NativeTest do
  use ExUnit.Case, async: true

  test "native library loads" do
    assert BrainIndex.Native.nif_version() == "0.1.0"
  end
end
