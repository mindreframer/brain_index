defmodule BrainIndex.Storage.RecordTest do
  use ExUnit.Case, async: true
  alias BrainIndex.Storage.Record

  @records [{1, {:upsert, 7, <<0, 0, 128, 63>>, [{"kind", "kb"}]}}, {2, {:delete, 7}}]

  test "frame/decode roundtrip" do
    bin = @records |> Enum.map(&Record.frame/1) |> IO.iodata_to_binary()
    assert {@records, :ok} = Record.decode(bin)
  end

  test "truncation and bit flips stop decoding at the bad record" do
    [a, b] = Enum.map(@records, &Record.frame/1)
    good = byte_size(a)
    assert {[_], {:error, ^good}} = Record.decode(a <> binary_part(b, 0, byte_size(b) - 1))
    <<head::binary-size(good + 10), x, rest::binary>> = a <> b

    assert {[_], {:error, ^good}} =
             Record.decode(<<head::binary, Bitwise.bxor(x, 1), rest::binary>>)

    assert {[], {:error, 0}} = Record.decode(<<0, 0>>)
  end

  test "valid crc but non-record payload is rejected" do
    payload = :erlang.term_to_binary(:not_a_record)
    bin = <<byte_size(payload)::32, :erlang.crc32(payload)::32, payload::binary>>
    assert {[], {:error, 0}} = Record.decode(bin)
  end
end
