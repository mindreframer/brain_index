defmodule BrainIndex.Storage.File do
  @moduledoc """
  Production `BrainIndex.Storage`: a segmented write-ahead log plus checkpoints in one directory
  (ADR007).

      {BrainIndex.Collection, name: :brain, dim: 768, storage: {BrainIndex.Storage.File, dir: "/var/lib/brain"}}

  Options: `:dir` (required) and `:segment_bytes` (rotate segments past this size; default 64 MiB).

  Layout:

      checkpoint-<lsn>.brix   snapshot containing every op with LSN <= lsn
      wal-<first_lsn>.log     CRC-framed records, appended with datasync

  On open, a torn record at the end of the newest segment (a crash during append) is truncated.
  Corruption anywhere else returns `{:error, {:corrupt_wal, file, offset}}` and is never silently
  skipped.
  """
  @behaviour BrainIndex.Storage

  alias BrainIndex.Storage.Record

  @default_segment_bytes 64 * 1024 * 1024
  @pad 20

  @impl true
  def open(opts) do
    dir = Keyword.fetch!(opts, :dir) |> Path.expand()

    with :ok <- File.mkdir_p(dir),
         :ok <- remove_tmp_files(dir),
         :ok <- repair_tail(dir) do
      {:ok,
       %{
         dir: dir,
         segment_bytes: Keyword.get(opts, :segment_bytes, @default_segment_bytes),
         fd: nil,
         seg_size: 0,
         last_lsn: 0
       }}
    end
  end

  @impl true
  def load_checkpoint(%{dir: dir}) do
    case checkpoints(dir) |> List.last() do
      nil ->
        :none

      {lsn, path} ->
        case BrainIndex.load(path) do
          {:ok, index} -> {:ok, index, lsn}
          {:error, e} -> {:error, {:corrupt_checkpoint, path, e.reason}}
        end
    end
  end

  @impl true
  def replay(%{dir: dir} = s, after_lsn, acc, fun) do
    segments(dir)
    |> Enum.reduce_while({:ok, acc, after_lsn}, fn {_first, path}, {:ok, acc, last} ->
      case Record.decode(File.read!(path)) do
        {records, :ok} ->
          {acc, last} =
            Enum.reduce(records, {acc, last}, fn {lsn, _} = r, {acc, last} ->
              if lsn > after_lsn, do: {fun.(r, acc), max(last, lsn)}, else: {acc, max(last, lsn)}
            end)

          {:cont, {:ok, acc, last}}

        {_, {:error, offset}} ->
          {:halt, {:error, {:corrupt_wal, Path.basename(path), offset}}}
      end
    end)
    |> case do
      {:ok, acc, last} -> {:ok, acc, last, %{s | last_lsn: max(s.last_lsn, last)}}
      error -> error
    end
  end

  @impl true
  def append(s, []), do: {:ok, s}

  def append(s, [{first_lsn, _} | _] = records) do
    bin = records |> Enum.map(&Record.frame/1) |> IO.iodata_to_binary()
    {last_lsn, _} = List.last(records)

    with {:ok, s} <- ensure_segment(s, first_lsn),
         :ok <- :file.write(s.fd, bin),
         :ok <- :file.datasync(s.fd) do
      s = %{s | seg_size: s.seg_size + byte_size(bin), last_lsn: last_lsn}
      {:ok, if(s.seg_size >= s.segment_bytes, do: close_segment(s), else: s)}
    end
  end

  @impl true
  def checkpoint(s, index, lsn) do
    s = close_segment(s)
    path = Path.join(s.dir, "checkpoint-#{pad(lsn)}.brix")

    with :ok <- BrainIndex.save(index, path) do
      for {old, p} <- checkpoints(s.dir), old < lsn, do: File.rm!(p)
      delete_covered_segments(s, lsn)
      :ok = BrainIndex.Native.fsync_dir(s.dir) |> ok()
      {:ok, s}
    end
  end

  @impl true
  def log_bytes(%{dir: dir}) do
    dir |> segments() |> Enum.reduce(0, fn {_, p}, n -> n + File.stat!(p).size end)
  end

  @impl true
  def close(s) do
    close_segment(s)
    :ok
  end

  # --- internals ------------------------------------------------------------------------------

  defp ensure_segment(%{fd: nil} = s, first_lsn) do
    path = Path.join(s.dir, "wal-#{pad(first_lsn)}.log")

    with {:ok, fd} <- :file.open(path, [:append, :binary, :raw]),
         :ok <- ok(BrainIndex.Native.fsync_dir(s.dir)) do
      {:ok, %{s | fd: fd, seg_size: 0}}
    end
  end

  defp ensure_segment(s, _first_lsn), do: {:ok, s}

  defp close_segment(%{fd: nil} = s), do: s

  defp close_segment(s) do
    :ok = :file.close(s.fd)
    %{s | fd: nil, seg_size: 0}
  end

  # A segment is covered when every record in it has LSN <= lsn: either the next segment starts
  # at or below lsn + 1, or it is the newest segment and nothing above lsn was appended.
  defp delete_covered_segments(s, lsn) do
    segs = segments(s.dir)
    nexts = Enum.map(Enum.drop(segs, 1), &elem(&1, 0)) ++ [nil]

    for {{_first, path}, next} <- Enum.zip(segs, nexts),
        (next != nil and next <= lsn + 1) or (next == nil and s.last_lsn <= lsn),
        do: File.rm!(path)
  end

  defp repair_tail(dir) do
    case List.last(segments(dir)) do
      nil ->
        :ok

      {_first, path} ->
        case Record.decode(File.read!(path)) do
          {_, :ok} -> :ok
          {_, {:error, offset}} -> truncate(path, offset)
        end
    end
  end

  defp truncate(path, offset) do
    {:ok, fd} = :file.open(path, [:read, :write, :binary, :raw])
    {:ok, _} = :file.position(fd, offset)
    :ok = :file.truncate(fd)
    :ok = :file.datasync(fd)
    :file.close(fd)
  end

  defp remove_tmp_files(dir) do
    dir
    |> File.ls!()
    |> Enum.filter(&String.ends_with?(&1, ".tmp"))
    |> Enum.each(&File.rm!(Path.join(dir, &1)))
  end

  defp segments(dir), do: numbered(dir, ~r/^wal-(\d+)\.log$/)
  defp checkpoints(dir), do: numbered(dir, ~r/^checkpoint-(\d+)\.brix$/)

  defp numbered(dir, regex) do
    for name <- File.ls!(dir), [_, n] <- [Regex.run(regex, name)], into: [] do
      {String.to_integer(n), Path.join(dir, name)}
    end
    |> Enum.sort()
  end

  defp pad(n), do: n |> Integer.to_string() |> String.pad_leading(@pad, "0")

  defp ok({:ok, :ok}), do: :ok
  defp ok(:ok), do: :ok
  defp ok(other), do: {:error, other}
end
