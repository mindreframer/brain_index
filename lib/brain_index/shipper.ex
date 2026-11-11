if Code.ensure_loaded?(ObjectStoreX) do
  defmodule BrainIndex.Shipper do
    @moduledoc """
    Ships backups to object storage and restores them (ADR012). Works with any `ObjectStoreX`
    store: S3, GCS, Azure, a local folder (`ObjectStoreX.new(:local, path: ...)`) or memory.

    A backup is one object, `<prefix>/<UTC timestamp>.mdb`: the compacted LMDB file from
    `BrainIndex.backup/2`. It is uploaded as a multipart stream, so a half-finished upload is
    never visible. Keys sort by time, so the newest is last.

        {:ok, store} = ObjectStoreX.new(:local, path: "/mnt/backups")
        {:ok, key} = BrainIndex.Shipper.ship(index, store, "brain")
        {:ok, _deleted} = BrainIndex.Shipper.prune(store, "brain", keep: 7)
        {:ok, key} = BrainIndex.Shipper.latest(store, "brain")
        :ok = BrainIndex.Shipper.restore(store, key, "/var/lib/brain")

    Requires the optional `:objectstorex` dependency.
    """

    alias BrainIndex.Error

    @chunk 8 * 1024 * 1024

    @doc "Backs up `index` and uploads it under `prefix`. Returns the object key."
    @spec ship(BrainIndex.t(), term(), String.t()) :: {:ok, String.t()} | {:error, term()}
    def ship(index, store, prefix) do
      tmp = Path.join(System.tmp_dir!(), "brain_ship-#{System.unique_integer([:positive])}")
      key = "#{prefix}/#{stamp()}.mdb"

      try do
        with :ok <- BrainIndex.backup(index, tmp),
             :ok <-
               tmp
               |> Path.join("data.mdb")
               |> File.stream!(@chunk)
               |> ObjectStoreX.Stream.upload(store, key) do
          {:ok, key}
        end
      after
        File.rm_rf(tmp)
      end
    end

    @doc "Backup keys under `prefix`, oldest first."
    @spec list(term(), String.t()) :: {:ok, [String.t()]} | {:error, term()}
    def list(store, prefix) do
      keys =
        ObjectStoreX.Stream.list_stream(store, prefix: prefix)
        |> Enum.map(& &1.location)
        |> Enum.filter(&String.ends_with?(&1, ".mdb"))
        |> Enum.sort()

      {:ok, keys}
    rescue
      e -> {:error, Exception.message(e)}
    end

    @doc "The newest backup key under `prefix`, or `{:ok, nil}`."
    @spec latest(term(), String.t()) :: {:ok, String.t() | nil} | {:error, term()}
    def latest(store, prefix) do
      with {:ok, keys} <- list(store, prefix), do: {:ok, List.last(keys)}
    end

    @doc "Deletes all but the newest `keep:` backups under `prefix`. Returns how many it deleted."
    @spec prune(term(), String.t(), keyword()) :: {:ok, non_neg_integer()} | {:error, term()}
    def prune(store, prefix, opts) do
      keep = Keyword.fetch!(opts, :keep)

      with {:ok, keys} <- list(store, prefix) do
        old = Enum.drop(keys, -keep)

        Enum.reduce_while(old, {:ok, 0}, fn k, {:ok, n} ->
          case ObjectStoreX.delete(store, k) do
            :ok -> {:cont, {:ok, n + 1}}
            err -> {:halt, err}
          end
        end)
      end
    end

    @doc """
    Downloads backup `key` into directory `dir`, which must hold no index. The file appears
    atomically (download to a temp name, then rename). Open the result with `BrainIndex.open/2`.
    """
    @spec restore(term(), String.t(), Path.t()) :: :ok | {:error, term()}
    def restore(store, key, dir) do
      target = Path.join(dir, "data.mdb")
      part = target <> ".part"

      if File.exists?(target) do
        {:error,
         Error.exception(reason: :invalid_argument, message: "#{dir} already holds an index")}
      else
        File.mkdir_p!(dir)

        try do
          File.open!(part, [:write, :binary], fn f ->
            ObjectStoreX.Stream.download(store, key) |> Enum.each(&IO.binwrite(f, &1))
          end)

          File.rename(part, target)
        rescue
          e ->
            File.rm(part)
            {:error, Exception.message(e)}
        end
      end
    end

    # Sortable, unique: 20261111T144846.530428Z
    defp stamp do
      DateTime.utc_now()
      |> DateTime.to_iso8601(:basic)
      |> String.replace(~r/[^0-9TZ.]/, "")
    end
  end
end
