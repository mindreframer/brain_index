//! LMDB environment, tables and record encodings (ADR009).

use crate::error::{Error, Result};
use crate::metric::Metric;
use heed::types::Bytes;
use heed::{Database, Env, EnvFlags, EnvOpenOptions, RoTxn, WithoutTls};
use std::path::Path;

pub type Db = Database<Bytes, Bytes>;

impl From<heed::Error> for Error {
    fn from(e: heed::Error) -> Self {
        Error::new("io", format!("lmdb: {e}"))
    }
}

pub struct Store {
    pub env: Env<WithoutTls>,
    pub config: Db,
    pub vectors: Db,
    pub links: Db,
    pub nodes: Db,
    pub ids: Db,
    pub meta: Db,
    pub payloads: Db,
}

impl Store {
    pub fn open(dir: &Path, map_size: usize, sync: bool) -> Result<Self> {
        std::fs::create_dir_all(dir).map_err(|e| Error::new("io", e.to_string()))?;
        let mut opts = EnvOpenOptions::new().read_txn_without_tls();
        opts.map_size(map_size).max_dbs(8);
        // Vector search reads pages randomly: kernel readahead only wastes I/O and cache.
        let mut flags = EnvFlags::NO_READ_AHEAD;
        if !sync {
            flags |= EnvFlags::NO_SYNC;
        }
        // SAFETY: one Env per directory per process is enforced by the caller (BrainIndex.open).
        let env = unsafe {
            opts.flags(flags);
            opts.open(dir)?
        };
        env.clear_stale_readers()?;
        let mut w = env.write_txn()?;
        let mut db = |name| env.create_database::<Bytes, Bytes>(&mut w, Some(name));
        let (config, vectors, links, nodes, ids, meta, payloads) = (
            db("config")?,
            db("vectors")?,
            db("links")?,
            db("nodes")?,
            db("ids")?,
            db("meta")?,
            db("payloads")?,
        );
        w.commit()?;
        Ok(Self {
            env,
            config,
            vectors,
            links,
            nodes,
            ids,
            meta,
            payloads,
        })
    }
}

// ---------- keys ----------

#[inline]
pub fn node_key(n: u32) -> [u8; 4] {
    n.to_be_bytes()
}

#[inline]
pub fn link_key(n: u32, level: u8) -> [u8; 5] {
    let b = n.to_be_bytes();
    [b[0], b[1], b[2], b[3], level]
}

#[inline]
pub fn ext_key(e: u64) -> [u8; 8] {
    e.to_be_bytes()
}

// ---------- records ----------

/// Graph and configuration state, persisted under `config["header"]` in every write transaction.
#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub dim: usize,
    pub metric: Metric,
    pub m: usize,
    pub ef_construction: usize,
    pub rng: u64,
    pub entry: Option<u32>,
    pub max_level: usize,
    pub next_node: u32,
}

const HEADER_KEY: &[u8] = b"header";
const HEADER_VERSION: u8 = 1;

impl Header {
    pub fn encode(&self) -> Vec<u8> {
        let mut b = vec![HEADER_VERSION];
        b.extend((self.dim as u32).to_le_bytes());
        b.push(match self.metric {
            Metric::Cosine => 0,
            Metric::Lorentz => 1,
        });
        b.extend((self.m as u32).to_le_bytes());
        b.extend((self.ef_construction as u32).to_le_bytes());
        b.extend(self.rng.to_le_bytes());
        b.extend(self.entry.unwrap_or(u32::MAX).to_le_bytes());
        b.extend((self.max_level as u32).to_le_bytes());
        b.extend(self.next_node.to_le_bytes());
        b
    }

    pub fn decode(b: &[u8]) -> Result<Self> {
        let bad = || Error::new("corrupt_store", "invalid header record");
        if b.len() != 34 || b[0] != HEADER_VERSION {
            return Err(bad());
        }
        let u32_at = |i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
        let entry = u32_at(22);
        Ok(Self {
            dim: u32_at(1) as usize,
            metric: match b[5] {
                0 => Metric::Cosine,
                1 => Metric::Lorentz,
                _ => return Err(bad()),
            },
            m: u32_at(6) as usize,
            ef_construction: u32_at(10) as usize,
            rng: u64::from_le_bytes(b[14..22].try_into().unwrap()),
            entry: (entry != u32::MAX).then_some(entry),
            max_level: u32_at(26) as usize,
            next_node: u32_at(30),
        })
    }

    pub fn load(s: &Store, txn: &RoTxn) -> Result<Option<Self>> {
        s.config.get(txn, HEADER_KEY)?.map(Self::decode).transpose()
    }

    pub fn save(&self, s: &Store, txn: &mut heed::RwTxn) -> Result<()> {
        Ok(s.config.put(txn, HEADER_KEY, &self.encode())?)
    }
}

/// `nodes` value: external id, live flag, number of levels.
pub fn encode_node(ext: u64, live: bool, levels: u8) -> [u8; 10] {
    let mut b = [0u8; 10];
    b[..8].copy_from_slice(&ext.to_le_bytes());
    b[8] = live as u8;
    b[9] = levels;
    b
}

pub fn decode_node(b: &[u8]) -> Result<(u64, bool, u8)> {
    if b.len() != 10 {
        return Err(Error::new("corrupt_store", "invalid node record"));
    }
    Ok((
        u64::from_le_bytes(b[..8].try_into().unwrap()),
        b[8] == 1,
        b[9],
    ))
}

/// `meta` value: u16 count, then (u16 len, bytes) for each key and value.
pub fn encode_meta(meta: &[(String, String)]) -> Vec<u8> {
    let mut b = (meta.len() as u16).to_le_bytes().to_vec();
    for (k, v) in meta {
        for s in [k, v] {
            b.extend((s.len() as u16).to_le_bytes());
            b.extend(s.as_bytes());
        }
    }
    b
}

pub fn decode_meta(b: &[u8]) -> Result<Vec<(String, String)>> {
    let bad = || Error::new("corrupt_store", "invalid meta record");
    let mut pos = 0;
    let mut take = |n: usize| -> Result<&[u8]> {
        let s = b.get(pos..pos + n).ok_or_else(bad)?;
        pos += n;
        Ok(s)
    };
    let count = u16::from_le_bytes(take(2)?.try_into().unwrap());
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let mut pair = [String::new(), String::new()];
        for slot in pair.iter_mut() {
            let len = u16::from_le_bytes(take(2)?.try_into().unwrap()) as usize;
            *slot = String::from_utf8(take(len)?.to_vec()).map_err(|_| bad())?;
        }
        let [k, v] = pair;
        out.push((k, v));
    }
    Ok(out)
}

pub fn encode_f32s(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

pub fn decode_f32s(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_roundtrip() {
        let h = Header {
            dim: 768,
            metric: Metric::Lorentz,
            m: 16,
            ef_construction: 200,
            rng: 42,
            entry: Some(7),
            max_level: 3,
            next_node: 99,
        };
        assert_eq!(Header::decode(&h.encode()).unwrap(), h);
        let none = Header { entry: None, ..h };
        assert_eq!(Header::decode(&none.encode()).unwrap().entry, None);
        assert_eq!(
            decode_node(&encode_node(u64::MAX, true, 4)).unwrap(),
            (u64::MAX, true, 4)
        );
        let meta = vec![
            ("kind".to_string(), "kb".to_string()),
            ("lang".to_string(), "dé".to_string()),
        ];
        assert_eq!(decode_meta(&encode_meta(&meta)).unwrap(), meta);
        assert!(decode_meta(&[5, 0, 1]).is_err());
        assert!(Header::decode(&[1, 2, 3]).is_err());
    }
}
