# EPIC001 Plan — Shipper

1. Optional dependency `objectstorex ~> 0.3.0` (with a rustler override), and a smoke test with a
   memory store.
2. `BrainIndex.Shipper`, compiled only when ObjectStoreX is loaded.
3. Tests: three shipments to a folder store, prune to 2, restore the latest and check size and
   payload, refuse a second restore; empty prefix and memory store.
4. Demo: `mix brain.backup [--keep N]` ships to the `data/backups` folder store.
5. ADR012, README, CHANGELOG, version 0.5.0.
