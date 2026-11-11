defmodule BrainIndex.Storage.Record do
  @moduledoc false
  # Log framing (ADR007): <<size::32, crc32::32, payload::binary-size(size)>>,
  # payload = :erlang.term_to_binary({lsn, op}).

  @header 8
  @max_size 1_073_741_824

  @spec frame(BrainIndex.Storage.log_record()) :: binary()
  def frame(record) do
    payload = :erlang.term_to_binary(record)
    <<byte_size(payload)::32, :erlang.crc32(payload)::32, payload::binary>>
  end

  @doc "Decodes records. Returns `{records, :ok}` or `{records_before_error, {:error, offset}}`."
  @spec decode(binary()) :: {[BrainIndex.Storage.log_record()], :ok | {:error, non_neg_integer()}}
  def decode(bin), do: decode(bin, 0, [])

  defp decode(bin, offset, acc) do
    case bin do
      <<>> ->
        {Enum.reverse(acc), :ok}

      <<size::32, crc::32, payload::binary-size(size), rest::binary>> when size <= @max_size ->
        with true <- :erlang.crc32(payload) == crc,
             {:ok, {lsn, _op} = record} when is_integer(lsn) <- safe_term(payload) do
          decode(rest, offset + @header + size, [record | acc])
        else
          _ -> {Enum.reverse(acc), {:error, offset}}
        end

      _ ->
        {Enum.reverse(acc), {:error, offset}}
    end
  end

  defp safe_term(payload) do
    {:ok, :erlang.binary_to_term(payload, [:safe])}
  rescue
    ArgumentError -> :error
  end
end
