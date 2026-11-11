defmodule BrainIndex.StorageContract do
  @moduledoc """
  Behaviour tests every `BrainIndex.Storage` backend must pass.

      use BrainIndex.StorageContract, backend: MyBackend
      # plus a `setup` returning %{opts: keyword} usable for repeated open/1 (reopen = restart)
  """

  defmacro __using__(backend: backend) do
    quote do
      @backend unquote(backend)

      defp up(id), do: {:upsert, id, BrainIndex.encode([1.0, id * 1.0]), [{"k", "v"}]}

      defp replay_all(s, after_lsn \\ 0) do
        {:ok, acc, last, s} = @backend.replay(s, after_lsn, [], &[&1 | &2])
        {Enum.reverse(acc), last, s}
      end

      test "append then replay returns records in order", %{opts: opts} do
        {:ok, s} = @backend.open(opts)
        {:ok, s} = @backend.append(s, [{1, up(1)}, {2, up(2)}])
        {:ok, s} = @backend.append(s, [{3, {:delete, 1}}])
        assert {[{1, _}, {2, _}, {3, {:delete, 1}}], 3, _} = replay_all(s)
        assert {[{3, _}], 3, _} = replay_all(s, 2)
        assert @backend.log_bytes(s) > 0
      end

      test "records survive reopen", %{opts: opts} do
        {:ok, s} = @backend.open(opts)
        {:ok, s} = @backend.append(s, [{1, up(1)}])
        :ok = @backend.close(s)
        {:ok, s2} = @backend.open(opts)
        assert {[{1, {:upsert, 1, _, [{"k", "v"}]}}], 1, _} = replay_all(s2)
      end

      test "empty store has no checkpoint and replays nothing", %{opts: opts} do
        {:ok, s} = @backend.open(opts)
        assert :none = @backend.load_checkpoint(s)
        assert {[], 0, _} = replay_all(s)
      end

      test "checkpoint captures the index and drops covered records", %{opts: opts} do
        {:ok, s} = @backend.open(opts)
        {:ok, s} = @backend.append(s, [{1, up(1)}, {2, up(2)}])
        {:ok, index} = BrainIndex.new(dim: 2)
        :ok = BrainIndex.insert(index, 1, [1.0, 1.0])
        :ok = BrainIndex.insert(index, 2, [1.0, 2.0])
        {:ok, s} = @backend.checkpoint(s, index, 2)
        {:ok, s} = @backend.append(s, [{3, up(3)}])
        :ok = @backend.close(s)

        {:ok, s} = @backend.open(opts)
        assert {:ok, restored, 2} = @backend.load_checkpoint(s)
        assert BrainIndex.size(restored) == 2
        assert {[{3, _}], 3, _} = replay_all(s, 2)
      end
    end
  end
end
