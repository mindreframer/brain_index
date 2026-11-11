defmodule BrainIndex.Storage.MemoryTest do
  use ExUnit.Case, async: true
  use BrainIndex.StorageContract, backend: BrainIndex.Storage.Memory

  setup do
    {:ok, agent} = BrainIndex.Storage.Memory.start()
    on_exit(fn -> Process.exit(agent, :kill) end)
    %{opts: [agent: agent]}
  end
end
