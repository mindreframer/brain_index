defmodule BrainIndex.PersistenceTest do
  use ExUnit.Case, async: true
  alias BrainIndex.Error

  @moduletag :tmp_dir

  defp vec(i), do: [1.0, :math.sin(i), :math.cos(i), rem(i, 5) * 1.0]

  test "payloads, reopen of an open directory, config validation, backup", %{tmp_dir: dir} do
    {:ok, index} = BrainIndex.open(dir, dim: 4)

    :ok =
      BrainIndex.insert_many(
        index,
        for(i <- 1..50, do: {i, vec(i), %{"p" => rem(i, 2)}, "chunk #{i}"})
      )

    :ok = BrainIndex.insert(index, 51, vec(51))
    {:ok, true} = BrainIndex.delete(index, 7)

    assert {:ok, [{3, _, "chunk 3"} | _]} = BrainIndex.search(index, vec(3), 3, payload: true)

    assert {:ok, [{51, _, nil}]} =
             BrainIndex.search(index, vec(51), 1, payload: true, exact: true)

    assert {:ok, "chunk 9"} = BrainIndex.get_payload(index, 9)
    assert {:ok, nil} = BrainIndex.get_payload(index, 7)

    # Opening the same directory again returns the same live index.
    {:ok, again} = BrainIndex.open(dir, dim: 4)
    assert BrainIndex.size(again) == 50
    assert {:error, %Error{reason: :dimension_mismatch}} = BrainIndex.open(dir, dim: 5)

    assert {:error, %Error{reason: :invalid_argument}} =
             BrainIndex.open(dir, dim: 4, metric: :lorentz)

    backup = Path.join(dir, "backup")
    :ok = BrainIndex.backup(index, backup)
    {:ok, copy} = BrainIndex.open(backup, dim: 4)

    assert BrainIndex.search(copy, vec(9), 5, payload: true) ==
             BrainIndex.search(index, vec(9), 5, payload: true)
  end

  test "payload validation" do
    {:ok, index} = BrainIndex.new(dim: 2)

    assert {:error, %Error{reason: :invalid_argument, message: "item 0: payload" <> _}} =
             BrainIndex.insert(index, 1, [1.0, 0.0], %{}, %{not: :binary})
  end

  @tag timeout: 60_000
  test "SIGKILL of the writing OS process loses no acknowledged write", %{tmp_dir: dir} do
    script = """
    {:ok, i} = BrainIndex.open(#{inspect(dir)}, dim: 4)
    for n <- 1..10_000 do
      :ok = BrainIndex.insert(i, n, [1.0, :math.sin(n), :math.cos(n), 0.5], %{}, "p\#{n}")
      IO.puts("acked \#{n}")
    end
    """

    port =
      Port.open({:spawn_executable, System.find_executable("mix")}, [
        :binary,
        :exit_status,
        {:line, 256},
        args: [
          "run",
          "--no-compile",
          "--no-start",
          "-e",
          "Application.ensure_all_started(:brain_index); " <> script
        ],
        env: [{~c"MIX_ENV", ~c"test"}]
      ])

    {:os_pid, os_pid} = Port.info(port, :os_pid)
    acked = wait_for_acks(port, 200)
    System.cmd("kill", ["-9", to_string(os_pid)])
    assert_receive {^port, {:exit_status, _}}, 10_000

    {:ok, index} = BrainIndex.open(dir, dim: 4)
    size = BrainIndex.size(index)
    assert size >= acked, "acked #{acked} but only #{size} survived"

    for n <- [1, div(acked, 2), acked],
        do: assert({:ok, "p" <> _} = BrainIndex.get_payload(index, n))
  end

  defp wait_for_acks(port, min) do
    receive do
      {^port, {:data, {:eol, "acked " <> n}}} ->
        n = String.to_integer(n)
        if n >= min, do: n, else: wait_for_acks(port, min)

      {^port, {:data, _}} ->
        wait_for_acks(port, min)

      {^port, {:exit_status, s}} ->
        flunk("writer exited early with #{s}")
    after
      30_000 -> flunk("no acks from writer")
    end
  end
end
