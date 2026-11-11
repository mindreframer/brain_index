# Usage: mix run bench/bench.exs [n] [dim]
# Reports build throughput, search latency (plain / filtered / exact) and snapshot size+timings.
{n, dim} =
  case System.argv() do
    [n, dim] -> {String.to_integer(n), String.to_integer(dim)}
    _ -> {10_000, 384}
  end

:rand.seed(:exsss, {1, 2, 3})
vec = fn -> for(_ <- 1..dim, into: <<>>, do: <<:rand.normal()::float-32-little>>) end
items = for i <- 1..n, do: {i, vec.(), %{"tag" => if(rem(i, 10) == 0, do: "rare", else: "common")}}
queries = for _ <- 1..500, do: vec.()

{:ok, index} = BrainIndex.new(dim: dim)
{build_us, :ok} = :timer.tc(fn -> BrainIndex.insert_many(index, items) end)

latency = fn opts ->
  times =
    for q <- queries do
      {us, {:ok, _}} = :timer.tc(fn -> BrainIndex.search(index, q, 10, opts) end)
      us
    end
    |> Enum.sort()

  {Enum.at(times, div(length(times), 2)), Enum.at(times, trunc(length(times) * 0.99))}
end

path = Path.join(System.tmp_dir!(), "bench.brix")
{save_us, :ok} = :timer.tc(fn -> BrainIndex.save(index, path) end)
{load_us, {:ok, _}} = :timer.tc(fn -> BrainIndex.load(path) end)

IO.puts("n=#{n} dim=#{dim}")
IO.puts("build: #{Float.round(build_us / 1.0e6, 2)} s (#{round(n / (build_us / 1.0e6))} vectors/s)")

for {label, opts} <- [
      {"hnsw k=10", []},
      {"hnsw k=10, filter 10%", [filter: %{"tag" => "rare"}, brute_force_threshold: 0]},
      {"brute-force filter 10%", [filter: %{"tag" => "rare"}]},
      {"exact k=10", [exact: true]}
    ] do
  {p50, p99} = latency.(opts)
  IO.puts("#{String.pad_trailing(label, 24)} p50 #{p50} µs  p99 #{p99} µs")
end

IO.puts("snapshot: #{div(File.stat!(path).size, 1024)} KiB, save #{div(save_us, 1000)} ms, load #{div(load_us, 1000)} ms")
File.rm!(path)
