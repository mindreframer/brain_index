# EPIC001 Spec — Shipper

- `ship/3 -> {:ok, key}`: one object per backup, `<prefix>/<timestamp>.mdb`, uploaded atomically.
- `list/2` returns keys oldest first; `latest/2` returns the newest key or nil; `prune/3` with
  `keep:` returns the number deleted.
- `restore/3` writes `dir/data.mdb` atomically and refuses an existing index. The result opens
  with `open/2` with all data and payloads.
- Tests use a local folder store and a memory store: no network, under 1 s.
