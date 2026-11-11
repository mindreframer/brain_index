//! Snapshot persistence (ADR005): versioned little-endian `BRIX` format with an FNV-1a 64 trailer,
//! written atomically (tmp file + fsync + rename). Every read is bounds-checked; malformed input
//! yields `corrupt_snapshot`, never a panic.

use crate::error::{Error, Result};
use crate::hnsw::{Params, MAX_LEVEL};
use crate::index::Index;
use crate::metric::Metric;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

const MAGIC: &[u8; 4] = b"BRIX";
const VERSION: u32 = 1;

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ *b as u64).wrapping_mul(0x0100_0000_01b3)
    })
}

fn corrupt(what: &str) -> Error {
    Error::new("corrupt_snapshot", format!("corrupt snapshot: {what}"))
}

fn io(e: std::io::Error) -> Error {
    Error::new("io", e.to_string())
}

// ---------- encoding ----------

struct Out(Vec<u8>);

impl Out {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn str(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.0.extend_from_slice(s.as_bytes());
    }
}

pub fn encode(idx: &Index) -> Vec<u8> {
    let n = idx.ext_ids.len();
    let mut o = Out(Vec::with_capacity(16 + idx.vectors.len() * 4 + n * 64));
    o.0.extend_from_slice(MAGIC);
    o.u32(VERSION);
    o.u32(idx.dim as u32);
    o.u8(match idx.metric {
        Metric::Cosine => 0,
        Metric::Lorentz => 1,
    });
    let p = &idx.graph.params;
    o.u32(p.m as u32);
    o.u32(p.ef_construction as u32);
    o.u64(p.seed);
    o.u64(idx.graph.rng);
    o.u32(idx.graph.entry.unwrap_or(u32::MAX));
    o.u32(idx.graph.max_level as u32);
    o.u32(n as u32);
    for x in &idx.vectors {
        o.0.extend_from_slice(&x.to_le_bytes());
    }
    for node in 0..n {
        o.u64(idx.ext_ids[node]);
        o.u8(u8::from(
            idx.by_ext.get(&idx.ext_ids[node]) == Some(&(node as u32)),
        ));
        let meta = &idx.node_meta[node];
        o.u32(meta.len() as u32);
        for (k, v) in meta {
            o.str(k);
            o.str(v);
        }
        let levels = &idx.graph.links[node];
        o.u8(levels.len() as u8);
        for links in levels {
            o.u32(links.len() as u32);
            links.iter().for_each(|&l| o.u32(l));
        }
    }
    let sum = fnv1a(&o.0);
    o.u64(sum);
    o.0
}

pub fn save(idx: &Index, path: &Path) -> Result<()> {
    let bytes = encode(idx);
    let tmp = path.with_extension("brix.tmp");
    let mut f = std::fs::File::create(&tmp).map_err(io)?;
    f.write_all(&bytes).map_err(io)?;
    f.sync_all().map_err(io)?;
    drop(f);
    std::fs::rename(&tmp, path).map_err(io)
}

// ---------- decoding ----------

struct In<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> In<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&e| e <= self.b.len())
            .ok_or_else(|| corrupt("truncated"))?;
        let s = &self.b[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn str(&mut self) -> Result<String> {
        let len = self.u32()? as usize;
        String::from_utf8(self.take(len)?.to_vec()).map_err(|_| corrupt("invalid utf-8"))
    }
}

pub fn decode(bytes: &[u8]) -> Result<Index> {
    if bytes.len() < 12 || &bytes[..4] != MAGIC {
        return Err(corrupt("bad magic"));
    }
    let (body, trailer) = bytes.split_at(bytes.len() - 8);
    if fnv1a(body) != u64::from_le_bytes(trailer.try_into().unwrap()) {
        return Err(corrupt("checksum mismatch"));
    }
    let mut r = In { b: body, pos: 4 };
    if r.u32()? != VERSION {
        return Err(corrupt("unsupported version"));
    }
    let dim = r.u32()? as usize;
    let metric = match r.u8()? {
        0 => Metric::Cosine,
        1 => Metric::Lorentz,
        _ => return Err(corrupt("unknown metric")),
    };
    let params = Params {
        m: r.u32()? as usize,
        ef_construction: r.u32()? as usize,
        seed: r.u64()?,
    };
    let rng = r.u64()?;
    let entry = r.u32()?;
    let max_level = r.u32()? as usize;
    let n = r.u32()? as usize;
    let mut idx =
        Index::with_params(dim, metric, params).map_err(|_| corrupt("invalid parameters"))?;

    let raw = r.take(
        n.checked_mul(dim * 4)
            .ok_or_else(|| corrupt("size overflow"))?,
    )?;
    idx.vectors = raw
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect();
    let mut by_ext = HashMap::with_capacity(n);
    for node in 0..n as u32 {
        let ext = r.u64()?;
        let live = r.u8()? == 1;
        let pairs = r.u32()? as usize;
        let mut meta = Vec::with_capacity(pairs.min(1024));
        for _ in 0..pairs {
            meta.push((r.str()?, r.str()?));
        }
        let levels = r.u8()? as usize;
        if levels == 0 || levels > MAX_LEVEL + 1 {
            return Err(corrupt("invalid level count"));
        }
        let mut node_links = Vec::with_capacity(levels);
        for _ in 0..levels {
            let count = r.u32()? as usize;
            let mut links = Vec::with_capacity(count.min(1024));
            for _ in 0..count {
                let l = r.u32()?;
                if l as usize >= n {
                    return Err(corrupt("link out of range"));
                }
                links.push(l);
            }
            node_links.push(links);
        }
        if live {
            if by_ext.insert(ext, node).is_some() {
                return Err(corrupt("duplicate live id"));
            }
            idx.add_postings(node, &meta);
        }
        idx.ext_ids.push(ext);
        idx.node_meta.push(meta);
        idx.graph.links.push(node_links);
    }
    if r.pos != body.len() {
        return Err(corrupt("trailing bytes"));
    }
    idx.by_ext = by_ext;
    idx.graph.max_level = max_level;
    idx.graph.rng = rng;
    if n > 0 {
        if entry as usize >= n || max_level >= idx.graph.links[entry as usize].len() {
            return Err(corrupt("invalid entry point"));
        }
        idx.graph.entry = Some(entry);
    }
    // Every link must point at a node that has the referenced level.
    for levels in &idx.graph.links {
        for (l, links) in levels.iter().enumerate() {
            if links
                .iter()
                .any(|&t| idx.graph.links[t as usize].len() <= l)
            {
                return Err(corrupt("link to missing level"));
            }
        }
    }
    Ok(idx)
}

pub fn load(path: &Path) -> Result<Index> {
    decode(&std::fs::read(path).map_err(io)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::tests::{filter, ids, meta};
    use crate::index::SearchOpts;

    fn sample() -> Index {
        let mut i = Index::new(4, Metric::Cosine).unwrap();
        for n in 0..200u64 {
            let v = [
                (n as f32).sin(),
                (n as f32 * 0.7).cos(),
                1.0,
                (n % 7) as f32,
            ];
            let kind = if n % 3 == 0 { "kb" } else { "ticket" };
            i.insert(n * 10, &v, meta(&[("kind", kind)])).unwrap();
        }
        i.delete(30);
        i.insert(40, &[0.0, 1.0, 0.0, 0.0], meta(&[("kind", "kb")]))
            .unwrap();
        i
    }

    #[test]
    fn roundtrip_preserves_search_results() {
        let a = sample();
        let b = decode(&encode(&a)).unwrap();
        assert_eq!((a.len(), a.deleted()), (b.len(), b.deleted()));
        let o = SearchOpts {
            k: 10,
            ..Default::default()
        };
        for q in [[1.0, 0.0, 0.5, 2.0], [0.0, 1.0, 0.0, 0.0]] {
            for f in [vec![], filter(&[("kind", &["kb"])])] {
                assert_eq!(a.search(&q, &f, &o).unwrap(), b.search(&q, &f, &o).unwrap());
            }
        }
        assert!(!ids(&b
            .search(&[1.0, 0.0, 0.5, 2.0], &vec![], &SearchOpts { k: 300, ..o })
            .unwrap())
        .contains(&30));
        assert_eq!(encode(&a), encode(&b), "encoding is canonical");
    }

    #[test]
    fn every_corruption_is_detected() {
        let bytes = encode(&sample());
        for i in (0..bytes.len()).step_by(97) {
            let mut c = bytes.clone();
            c[i] ^= 0x5A;
            assert_eq!(
                decode(&c).err().map(|e| e.reason),
                Some("corrupt_snapshot"),
                "byte {i}"
            );
        }
        for len in [0, 3, 11, bytes.len() / 2, bytes.len() - 1] {
            assert_eq!(
                decode(&bytes[..len]).err().map(|e| e.reason),
                Some("corrupt_snapshot")
            );
        }
    }

    #[test]
    fn empty_index_roundtrip_and_atomic_save() {
        let dir = std::env::temp_dir().join(format!("brix-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("empty.brix");
        save(&Index::new(3, Metric::Lorentz).unwrap(), &path).unwrap();
        let back = load(&path).unwrap();
        assert_eq!((back.len(), back.metric), (0, Metric::Lorentz));
        assert!(!path.with_extension("brix.tmp").exists());
        assert_eq!(
            load(&dir.join("missing.brix")).err().map(|e| e.reason),
            Some("io")
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
