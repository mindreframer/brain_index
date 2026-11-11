defmodule BrainIndex.Fixtures do
  @moduledoc """
  Spike oracle fixtures: 543 multilingual chunks and 150 labelled queries embedded with
  multilingual-e5-base (768D, cosine) and YARlabs v5 (129D, Lorentz).
  Regenerate with `test/support/export_fixtures.py`.
  """

  @dir Path.expand("../fixtures", __DIR__)

  def chunks do
    for line <- lines("chunks.tsv") do
      [id, doc_id, kind, lang] = String.split(line, "\t")
      %{id: String.to_integer(id), doc_id: doc_id, kind: kind, lang: lang}
    end
  end

  def queries do
    for line <- lines("queries.tsv") do
      [qid, kind, target, cross] = String.split(line, "\t")
      %{qid: qid, kind: kind, target: target, cross: cross == "1"}
    end
  end

  @doc "Vectors as lists of floats. model: :e5 | :v5, set: :chunks | :queries"
  def vectors(model, set) do
    bytes = dim(model) * 4
    bin = File.read!(Path.join(@dir, "#{model}_#{set}.f32"))
    for <<chunk::binary-size(^bytes) <- bin>>, do: for(<<x::float-32-little <- chunk>>, do: x)
  end

  def dim(:e5), do: 768
  def dim(:v5), do: 129

  def metric(:e5), do: :cosine
  def metric(:v5), do: :lorentz

  @doc "Doc-level recall@5: share of queries whose target doc appears among the first 5 distinct docs."
  def recall_at_5(results_per_query) do
    doc_of = Map.new(chunks(), &{&1.id, &1.doc_id})

    hits =
      Enum.zip(queries(), results_per_query)
      |> Enum.count(fn {q, hits} ->
        hits
        |> Enum.map(fn {id, _d} -> doc_of[id] end)
        |> Enum.uniq()
        |> Enum.take(5)
        |> Enum.member?(q.target)
      end)

    hits / length(queries())
  end

  defp lines(file), do: @dir |> Path.join(file) |> File.read!() |> String.split("\n", trim: true)
end
