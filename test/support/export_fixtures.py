"""Exports spike vectors to compact binary fixtures (little-endian f32) + TSV labels.
Usage (from spike root): python3 -I brain_index/test/support/export_fixtures.py <spike_root>"""
import json, struct, sys
root = sys.argv[1]; out = f"{root}/brain_index/test/fixtures"
L = lambda p: [json.loads(l) for l in open(f"{root}/data/{p}") if l.strip()]
def write_f32(path, rows):
    with open(path, "wb") as f:
        for v in rows: f.write(struct.pack(f"<{len(v)}f", *v))
e5c, e5q, v5c, v5q = L("emb_chunks.jsonl"), L("emb_queries.jsonl"), L("emb_chunks_v5.jsonl"), L("emb_queries_v5.jsonl")
assert [c["id"] for c in e5c] == [c["id"] for c in v5c] and [q["qid"] for q in e5q] == [q["qid"] for q in v5q]
write_f32(f"{out}/e5_chunks.f32", [c["v"] for c in e5c]); write_f32(f"{out}/e5_queries.f32", [q["v"] for q in e5q])
write_f32(f"{out}/v5_chunks.f32", [c["v"] for c in v5c]); write_f32(f"{out}/v5_queries.f32", [q["v"] for q in v5q])
with open(f"{out}/chunks.tsv", "w") as f:
    for c in e5c: f.write(f"{c['id']}\t{c['doc_id']}\t{c['kind']}\t{c['lang']}\n")
with open(f"{out}/queries.tsv", "w") as f:
    for q in e5q: f.write(f"{q['qid']}\t{q['kind']}\t{q['target']}\t{int(q['cross'])}\n")
print(len(e5c), len(e5c[0]["v"]), len(v5c[0]["v"]), len(e5q))
