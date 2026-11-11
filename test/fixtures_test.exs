defmodule BrainIndex.FixturesTest do
  use ExUnit.Case, async: true
  alias BrainIndex.Fixtures

  test "fixtures are consistent" do
    assert length(Fixtures.chunks()) == 543
    assert length(Fixtures.queries()) == 150
    assert [v | _] = Fixtures.vectors(:e5, :chunks)
    assert length(v) == 768
    assert length(Fixtures.vectors(:v5, :queries)) == 150
  end
end
