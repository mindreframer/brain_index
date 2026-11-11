defmodule BrainIndex.Storage.FileTest do
  use ExUnit.Case, async: true
  use BrainIndex.StorageContract, backend: BrainIndex.Storage.File

  alias BrainIndex.Storage.File, as: FS

  @moduletag :tmp_dir

  setup %{tmp_dir: dir}, do: %{opts: [dir: dir], dir: dir}

  defp seg_files(dir),
    do: dir |> File.ls!() |> Enum.filter(&String.starts_with?(&1, "wal-")) |> Enum.sort()

  defp replay(opts) do
    {:ok, s} = FS.open(opts)
    {:ok, acc, last, _} = FS.replay(s, 0, [], &[elem(&1, 0) | &2])
    {Enum.reverse(acc), last}
  end

  test "segments rotate by size and replay across them", %{dir: dir} do
    {:ok, s} = FS.open(dir: dir, segment_bytes: 200)
    s = Enum.reduce(1..20, s, fn i, s -> elem(FS.append(s, [{i, up(i)}]), 1) end)
    FS.close(s)
    assert length(seg_files(dir)) > 3
    assert replay(dir: dir) == {Enum.to_list(1..20), 20}
  end

  test "torn tail from a crash mid-append is truncated on open", %{dir: dir} do
    {:ok, s} = FS.open(dir: dir)
    {:ok, s} = FS.append(s, [{1, up(1)}, {2, up(2)}])
    FS.close(s)
    [seg] = seg_files(dir)
    path = Path.join(dir, seg)
    good = File.stat!(path).size
    frame = BrainIndex.Storage.Record.frame({3, up(3)})
    File.write!(path, binary_part(frame, 0, 11), [:append])

    assert replay(dir: dir) == {[1, 2], 2}
    assert File.stat!(path).size == good
    # Appending after repair continues cleanly.
    {:ok, s} = FS.open(dir: dir)
    {:ok, s} = FS.append(s, [{3, up(3)}])
    FS.close(s)
    assert replay(dir: dir) == {[1, 2, 3], 3}
  end

  test "corruption in a closed segment fails loudly", %{dir: dir} do
    {:ok, s} = FS.open(dir: dir, segment_bytes: 1)
    s = Enum.reduce(1..3, s, fn i, s -> elem(FS.append(s, [{i, up(i)}]), 1) end)
    FS.close(s)
    [first | _] = seg_files(dir)
    path = Path.join(dir, first)
    <<head::binary-size(20), b, rest::binary>> = File.read!(path)
    File.write!(path, <<head::binary, Bitwise.bxor(b, 0xFF), rest::binary>>)
    {:ok, s} = FS.open(dir: dir)
    assert {:error, {:corrupt_wal, ^first, 0}} = FS.replay(s, 0, [], &[&1 | &2])
  end

  test "checkpoint removes covered segments and old checkpoints", %{dir: dir} do
    {:ok, index} = BrainIndex.new(dim: 2)
    {:ok, s} = FS.open(dir: dir, segment_bytes: 100)
    s = Enum.reduce(1..10, s, fn i, s -> elem(FS.append(s, [{i, up(i)}]), 1) end)
    {:ok, s} = FS.checkpoint(s, index, 10)
    {:ok, s} = FS.append(s, [{11, up(11)}])
    {:ok, s} = FS.checkpoint(s, index, 11)
    {:ok, s} = FS.append(s, [{12, up(12)}])
    FS.close(s)
    files = File.ls!(dir) |> Enum.sort()
    assert files == ["checkpoint-00000000000000000011.brix", "wal-00000000000000000012.log"]
  end

  test "leftover tmp files are removed on open", %{dir: dir} do
    File.write!(Path.join(dir, "checkpoint-00000000000000000005.brix.tmp"), "partial")
    {:ok, _} = FS.open(dir: dir)
    assert File.ls!(dir) == []
  end
end
