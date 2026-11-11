# Usage: MIX_ENV=prod mix run bench/bench.exs [n] [dim]
# Durable (fsync) LMDB index: build, single-insert latency and disk bytes written per insert,
# search latency, cold reopen, process RSS. Disk bytes via bench/diskio.py (macOS).
{n, dim} =
  case System.argv() do
    [n, dim] -> {String.to_integer(n), String.to_integer(dim)}
    _ -> {10_000, 384}
  end

diskio = fn ->
  {out, 0} = System.cmd("python3", ["-I", Path.expand("diskio.py", __DIR__), System.pid()])
  [w, rss] = out |> String.trim() |> String.split() |> Enum.map(&String.to_integer/1)
  {w, rss}
end

:rand.seed(:exsss, {1, 2, 3})
vec = fn -> for(_ <- 1..dim, into: <<>>, do: <<:rand.normal()::float-32-little>>) end
tag = fn i -> %{"tag" => if(rem(i, 10) == 0, do: "rare", else: "common")} end
dir = Path.join(System.tmp_dir!(), "brain_bench_#{System.unique_integer([:positive])}")
{:ok, index} = BrainIndex.open(dir, dim: dim)

{w0, _} = diskio.()
{build_us, :ok} =
  :timer.tc(fn ->
    1..n |> Enum.chunk_every(1000) |> Enum.each(fn ids -> :ok = BrainIndex.insert_many(index, for(i <- ids, do: {i, vec.(), tag.(i), "payload #{i}"})) end)
  end)
{w1, _} = diskio.()

singles = 200
single_us =
  for i <- (n + 1)..(n + singles) do
    v = vec.()
    {us, :ok} = :timer.tc(fn -> BrainIndex.insert(index, i, v, tag.(i), "payload #{i}") end)
    us
  end
{w2, _} = diskio.()

{:ok, writer} = BrainIndex.Writer.start_link(name: :bench_writer, index: index)
{grouped_us, _} =
  :timer.tc(fn ->
    (n + singles + 1)..(n + 2 * singles)
    |> Task.async_stream(fn i -> :ok = BrainIndex.Writer.insert(writer, i, vec.(), tag.(i), "payload #{i}") end, max_concurrency: 50)
    |> Stream.run()
  end)
{w3, rss} = diskio.()
%{commits: commits} = BrainIndex.Writer.stats(writer)

pct = fn xs, p -> xs |> Enum.sort() |> Enum.at(min(length(xs) - 1, trunc(length(xs) * p))) end
queries = for _ <- 1..500, do: vec.()
latency = fn opts ->
  t = for q <- queries, do: elem(:timer.tc(fn -> {:ok, _} = BrainIndex.search(index, q, 10, opts) end), 0)
  {pct.(t, 0.5), pct.(t, 0.99)}
end

backup = dir <> "_copy"
:ok = BrainIndex.backup(index, backup)
{reopen_us, {:ok, copy}} = :timer.tc(fn -> BrainIndex.open(backup, dim: dim) end)
true = BrainIndex.size(copy) == n + 2 * singles

mb = fn b -> Float.round(b / 1_048_576, 1) end
file = File.stat!(Path.join(dir, "data.mdb")).size
IO.puts("n=#{n} dim=#{dim}  (durable: fsync per commit)")
IO.puts("build (batches of 1000): #{Float.round(build_us / 1.0e6, 2)} s, #{round(n / (build_us / 1.0e6))} vectors/s, disk written #{mb.(w1 - w0)} MiB (raw vectors #{mb.(n * dim * 4)} MiB)")
IO.puts("single insert + commit: p50 #{pct.(single_us, 0.5)} µs, p99 #{pct.(single_us, 0.99)} µs, disk written #{div(w2 - w1, singles)} bytes/insert")
IO.puts("group commit (Writer, 50 concurrent callers): #{singles} inserts in #{div(grouped_us, 1000)} ms, #{commits} commits, disk written #{div(w3 - w2, singles)} bytes/insert")
for {label, opts} <- [{"hnsw k=10", []}, {"hnsw k=10 + payload", [payload: true]}, {"filter 10% (brute force)", [filter: %{"tag" => "rare"}]}, {"exact k=10", [exact: true]}] do
  {p50, p99} = latency.(opts)
  IO.puts("#{String.pad_trailing(label, 26)} p50 #{p50} µs  p99 #{p99} µs")
end
IO.puts("data.mdb #{mb.(file)} MiB; cold reopen (incl. postings rebuild) #{div(reopen_us, 1000)} ms; process RSS #{mb.(rss)} MiB")
File.rm_rf!(dir)
File.rm_rf!(backup)
