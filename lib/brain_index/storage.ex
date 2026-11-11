defmodule BrainIndex.Storage do
  @moduledoc """
  Durability backend for `BrainIndex.Collection` (ADR007).

  A backend persists an ordered log of records and occasional checkpoints. `append/2` must be
  durable when it returns. Implementations: `BrainIndex.Storage.File` (production) and
  `BrainIndex.Storage.Memory` (tests).
  """

  @type lsn :: non_neg_integer()
  @type op ::
          {:upsert, BrainIndex.id(), binary(), [{String.t(), String.t()}]}
          | {:delete, BrainIndex.id()}
  @type log_record :: {lsn(), op()}
  @type state :: term()

  @callback open(opts :: keyword()) :: {:ok, state()} | {:error, term()}
  @callback load_checkpoint(state()) :: {:ok, BrainIndex.t(), lsn()} | :none | {:error, term()}
  @callback replay(state(), after_lsn :: lsn(), acc, (log_record(), acc -> acc)) ::
              {:ok, acc, last_lsn :: lsn(), state()} | {:error, term()}
            when acc: term()
  @callback append(state(), [log_record()]) :: {:ok, state()} | {:error, term()}
  @callback checkpoint(state(), BrainIndex.t(), lsn()) :: {:ok, state()} | {:error, term()}
  @callback log_bytes(state()) :: non_neg_integer()
  @callback close(state()) :: :ok
end
