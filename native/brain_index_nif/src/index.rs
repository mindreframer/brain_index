//! The index: LMDB-resident vectors and graph (ADR009), in-memory live set and filter postings,
//! payloads (ADR010). One writer at a time; searches run concurrently on MVCC snapshots.

use crate::bitset::Bitset;
use crate::error::{Error, Result};
use crate::hnsw::{self, View};
use crate::metric::Metric;
use crate::store::{
    decode_meta, decode_node, encode_f32s, encode_meta, encode_node, ext_key, node_key, Header,
    Store,
};
use crate::topk::{Scored, TopK};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

/// Metadata of one vector: exact-match string pairs (ADR004).
pub type Meta = Vec<(String, String)>;
/// Filter: AND across keys, OR across the values of one key.
pub type Filter = Vec<(String, Vec<String>)>;
/// One item for `insert_many`.
pub type Item = (u64, Vec<f32>, Meta, Option<Vec<u8>>);
/// Search hit: external id, distance, optional payload.
pub type Hit = (u64, f32, Option<Vec<u8>>);

#[derive(Debug, Clone)]
pub struct Config {
    pub dim: usize,
    pub metric: Metric,
    pub m: usize,
    pub ef_construction: usize,
    /// Upper bound for the data file (sparse; it grows with data).
    pub map_size: usize,
    /// fsync on commit (ADR009). Only disable for throwaway or rebuildable indexes.
    pub sync: bool,
}

impl Config {
    pub fn new(dim: usize, metric: Metric) -> Self {
        Self {
            dim,
            metric,
            m: 16,
            ef_construction: 200,
            map_size: 1 << 40,
            sync: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchOpts {
    pub k: usize,
    pub ef: usize,
    pub exact: bool,
    /// Filtered searches whose allowed set is at most this large run as exact brute force.
    pub brute_force_threshold: usize,
    pub payload: bool,
}

impl Default for SearchOpts {
    fn default() -> Self {
        Self {
            k: 10,
            ef: 100,
            exact: false,
            brute_force_threshold: 4096,
            payload: false,
        }
    }
}

/// In-memory acceleration state, derived from LMDB on open and updated after each commit.
#[derive(Default)]
struct Mem {
    live: Bitset,
    postings: HashMap<String, HashMap<String, Bitset>>,
}

impl Mem {
    fn add(&mut self, node: u32, meta: &Meta) {
        self.live.set(node);
        for (k, v) in meta {
            self.postings
                .entry(k.clone())
                .or_default()
                .entry(v.clone())
                .or_default()
                .set(node);
        }
    }

    fn remove(&mut self, node: u32, meta: &Meta) {
        self.live.clear(node);
        for (k, v) in meta {
            if let Some(bits) = self.postings.get_mut(k).and_then(|m| m.get_mut(v)) {
                bits.clear(node);
            }
        }
    }

    /// Live nodes allowed by `filter`, or `None` for "all live nodes".
    fn allowed(&self, filter: &Filter) -> Option<Bitset> {
        let mut acc: Option<Bitset> = None;
        for (key, values) in filter {
            let mut any = Bitset::default();
            if let Some(by_value) = self.postings.get(key) {
                for v in values {
                    if let Some(bits) = by_value.get(v) {
                        any.or_with(bits);
                    }
                }
            }
            match acc.as_mut() {
                None => acc = Some(any),
                Some(a) => a.and_with(&any),
            }
        }
        acc.map(|mut a| {
            a.and_with(&self.live);
            a
        })
    }
}

enum Change {
    Added(u32, Meta),
    Removed(u32, Meta),
}

/// Deletes its directory when dropped (temporary indexes).
struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub struct Index {
    // Field order matters: the LMDB environment must close before the temp dir is removed.
    store: Store,
    mem: RwLock<Mem>,
    writer: Mutex<()>,
    _temp: Option<TempDir>,
}

fn poisoned<T>(_: T) -> Error {
    Error::new("internal", "index lock poisoned")
}

impl Index {
    /// Opens (or creates) a durable index in `dir`. An existing index must match `dim`/`metric`.
    pub fn open(dir: &Path, cfg: &Config) -> Result<Self> {
        Self::open_inner(dir, cfg, None)
    }

    /// A private index in a temp directory, removed when dropped. No fsync (throwaway by nature).
    pub fn temp(cfg: &Config) -> Result<Self> {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("brain_index-{}-{n}", std::process::id()));
        let cfg = Config {
            sync: false,
            map_size: cfg.map_size.min(1 << 34),
            ..cfg.clone()
        };
        Self::open_inner(&dir, &cfg, Some(TempDir(dir.clone())))
    }

    fn open_inner(dir: &Path, cfg: &Config, temp: Option<TempDir>) -> Result<Self> {
        if cfg.dim == 0 || cfg.dim > 65_536 {
            return Err(Error::invalid_argument("dim must be in 1..=65536"));
        }
        if !(2..=256).contains(&cfg.m) || cfg.ef_construction < 1 {
            return Err(Error::invalid_argument(
                "m must be in 2..=256 and ef_construction >= 1",
            ));
        }
        let store = Store::open(dir, cfg.map_size, cfg.sync)?;
        let existing = Header::load(&store, &*store.env.read_txn()?)?;
        match existing {
            Some(h) if h.dim != cfg.dim => return Err(Error::dimension_mismatch(h.dim, cfg.dim)),
            Some(h) if h.metric != cfg.metric => {
                return Err(Error::invalid_argument(format!(
                    "index uses metric {}",
                    h.metric.name()
                )))
            }
            Some(_) => {}
            None => {
                let h = Header {
                    dim: cfg.dim,
                    metric: cfg.metric,
                    m: cfg.m,
                    ef_construction: cfg.ef_construction,
                    rng: 0x5EED_B2A1_1DE7,
                    entry: None,
                    max_level: 0,
                    next_node: 0,
                };
                let mut w = store.env.write_txn()?;
                h.save(&store, &mut w)?;
                w.commit()?;
            }
        }
        let mem = Self::rebuild_mem(&store)?;
        Ok(Self {
            store,
            mem: RwLock::new(mem),
            writer: Mutex::new(()),
            _temp: temp,
        })
    }

    /// Errors unless `cfg` is compatible with this index (same dim and metric).
    pub fn check_config(&self, cfg: &Config) -> Result<()> {
        let h = self.header(&*self.store.env.read_txn()?)?;
        if h.dim != cfg.dim {
            return Err(Error::dimension_mismatch(h.dim, cfg.dim));
        }
        if h.metric != cfg.metric {
            return Err(Error::invalid_argument(format!(
                "index uses metric {}",
                h.metric.name()
            )));
        }
        Ok(())
    }

    fn rebuild_mem(s: &Store) -> Result<Mem> {
        let r = s.env.read_txn()?;
        let mut mem = Mem::default();
        for entry in s.meta.iter(&r)? {
            let (k, v) = entry?;
            let node = u32::from_be_bytes(
                k.try_into()
                    .map_err(|_| Error::new("corrupt_store", "bad meta key"))?,
            );
            mem.add(node, &decode_meta(v)?);
        }
        Ok(mem)
    }

    fn header(&self, txn: &heed::RoTxn) -> Result<Header> {
        Header::load(&self.store, txn)?.ok_or_else(|| Error::new("corrupt_store", "missing header"))
    }

    fn prepare(h: &Header, v: &[f32]) -> Result<Vec<f32>> {
        if v.len() != h.dim {
            return Err(Error::dimension_mismatch(h.dim, v.len()));
        }
        let mut v = v.to_vec();
        h.metric.prepare(&mut v)?;
        Ok(v)
    }

    pub fn insert(&self, ext: u64, v: &[f32], meta: Meta, payload: Option<Vec<u8>>) -> Result<()> {
        self.insert_many(vec![(ext, v.to_vec(), meta, payload)])
    }

    /// Inserts or replaces (upsert) a batch in one transaction: all items are validated first, so an
    /// invalid item leaves the index unchanged. Durable on return when `sync` is on.
    pub fn insert_many(&self, items: Vec<Item>) -> Result<()> {
        let _w = self.writer.lock().map_err(poisoned)?;
        let s = &self.store;
        let mut w = s.env.write_txn()?;
        let mut h = self.header(&w)?;
        let prepared = items
            .into_iter()
            .enumerate()
            .map(|(i, (id, v, meta, payload))| {
                Self::prepare(&h, &v)
                    .map(|v| (id, v, meta, payload))
                    .map_err(|e| Error::new(e.reason, format!("item {i}: {}", e.message)))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut changes = Vec::with_capacity(prepared.len());
        for (ext, v, meta, payload) in prepared {
            self.delete_in(&mut w, ext, &mut changes)?;
            if h.next_node == u32::MAX {
                return Err(Error::invalid_argument("index is full"));
            }
            let node = h.next_node;
            h.next_node += 1;
            let level = hnsw::random_level(&mut h);
            s.vectors.put(&mut w, &node_key(node), &encode_f32s(&v))?;
            s.nodes.put(
                &mut w,
                &node_key(node),
                &encode_node(ext, true, level as u8 + 1),
            )?;
            s.ids.put(&mut w, &ext_key(ext), &node_key(node))?;
            s.meta.put(&mut w, &node_key(node), &encode_meta(&meta))?;
            if let Some(p) = &payload {
                s.payloads.put(&mut w, &ext_key(ext), p)?;
            }
            hnsw::insert(&mut w, s, &mut h, node, &v, level)?;
            changes.push(Change::Added(node, meta));
        }
        h.save(s, &mut w)?;
        w.commit()?;
        self.apply(changes)
    }

    /// Tombstones `ext`: the node keeps routing searches but never matches. Returns whether it existed.
    pub fn delete(&self, ext: u64) -> Result<bool> {
        let _w = self.writer.lock().map_err(poisoned)?;
        let mut w = self.store.env.write_txn()?;
        let mut changes = Vec::new();
        let existed = self.delete_in(&mut w, ext, &mut changes)?;
        if existed {
            w.commit()?;
            self.apply(changes)?;
        }
        Ok(existed)
    }

    fn delete_in(&self, w: &mut heed::RwTxn, ext: u64, changes: &mut Vec<Change>) -> Result<bool> {
        let s = &self.store;
        let Some(node_bytes) = s.ids.get(w, &ext_key(ext))? else {
            return Ok(false);
        };
        let node = u32::from_be_bytes(
            node_bytes
                .try_into()
                .map_err(|_| Error::new("corrupt_store", "bad id record"))?,
        );
        let (_, _, levels) = decode_node(s.nodes.get(w, &node_key(node))?.unwrap_or(&[]))?;
        let meta = decode_meta(s.meta.get(w, &node_key(node))?.unwrap_or(&[0, 0]))?;
        s.ids.delete(w, &ext_key(ext))?;
        s.nodes
            .put(w, &node_key(node), &encode_node(ext, false, levels))?;
        s.meta.delete(w, &node_key(node))?;
        s.payloads.delete(w, &ext_key(ext))?;
        changes.push(Change::Removed(node, meta));
        Ok(true)
    }

    fn apply(&self, changes: Vec<Change>) -> Result<()> {
        let mut mem = self.mem.write().map_err(poisoned)?;
        for c in changes {
            match c {
                Change::Added(n, meta) => mem.add(n, &meta),
                Change::Removed(n, meta) => mem.remove(n, &meta),
            }
        }
        Ok(())
    }

    /// Unified search: exact or HNSW, optionally filtered (ADR004).
    pub fn search(&self, q: &[f32], filter: &Filter, o: &SearchOpts) -> Result<Vec<Hit>> {
        // Lock order: mem first, then snapshot. Every node visible in `mem` is then committed and
        // therefore present in the snapshot.
        let mem = self.mem.read().map_err(poisoned)?;
        let r = self.store.env.read_txn()?;
        let h = self.header(&r)?;
        let q = Self::prepare(&h, q)?;
        let v = View {
            txn: &r,
            s: &self.store,
            metric: h.metric,
        };
        let allowed = mem.allowed(filter);
        let hits = match &allowed {
            Some(bits) if o.exact || bits.count() <= o.brute_force_threshold => {
                brute_force(&v, &q, o.k, bits.iter())?
            }
            Some(bits) => hnsw::search(&v, &h, &q, o.k, o.ef, &|n| bits.contains(n))?,
            None if o.exact => brute_force(&v, &q, o.k, mem.live.iter())?,
            None => hnsw::search(&v, &h, &q, o.k, o.ef, &|n| mem.live.contains(n))?,
        };
        drop(mem);
        hits.into_iter()
            .map(|sc| {
                let (ext, _, _) =
                    decode_node(self.store.nodes.get(&r, &node_key(sc.id))?.unwrap_or(&[]))?;
                let payload = if o.payload {
                    self.store
                        .payloads
                        .get(&r, &ext_key(ext))?
                        .map(<[u8]>::to_vec)
                } else {
                    None
                };
                Ok((ext, sc.dist, payload))
            })
            .collect()
    }

    pub fn get_payload(&self, ext: u64) -> Result<Option<Vec<u8>>> {
        let r = self.store.env.read_txn()?;
        Ok(self
            .store
            .payloads
            .get(&r, &ext_key(ext))?
            .map(<[u8]>::to_vec))
    }

    /// (dim, metric, live, deleted, m, ef_construction)
    pub fn info(&self) -> Result<(usize, Metric, usize, usize, usize, usize)> {
        let mem = self.mem.read().map_err(poisoned)?;
        let h = self.header(&*self.store.env.read_txn()?)?;
        let live = mem.live.count();
        Ok((
            h.dim,
            h.metric,
            live,
            h.next_node as usize - live,
            h.m,
            h.ef_construction,
        ))
    }

    pub fn len(&self) -> usize {
        self.mem.read().map(|m| m.live.count()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Consistent, compacted online copy into `dir` (opens with `Index::open`).
    pub fn backup(&self, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir).map_err(|e| Error::new("io", e.to_string()))?;
        self.store
            .env
            .copy_to_path(dir.join("data.mdb"), heed::CompactionOption::Enabled)?;
        Ok(())
    }
}

fn brute_force(
    v: &View,
    q: &[f32],
    k: usize,
    nodes: impl Iterator<Item = u32>,
) -> Result<Vec<Scored>> {
    let mut top = TopK::new(k);
    for node in nodes {
        top.push(Scored {
            dist: v.dist(q, node)?,
            id: node,
        });
    }
    Ok(top.into_sorted())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn meta(pairs: &[(&str, &str)]) -> Meta {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    pub fn filter(pairs: &[(&str, &[&str])]) -> Filter {
        pairs
            .iter()
            .map(|(k, vs)| (k.to_string(), vs.iter().map(|v| v.to_string()).collect()))
            .collect()
    }

    pub fn ids(hits: &[Hit]) -> Vec<u64> {
        hits.iter().map(|h| h.0).collect()
    }

    pub fn opts(k: usize, exact: bool, threshold: usize) -> SearchOpts {
        SearchOpts {
            k,
            ef: 64,
            exact,
            brute_force_threshold: threshold,
            payload: false,
        }
    }

    pub fn random_vectors(n: usize, dim: usize, seed: u64) -> Vec<Vec<f32>> {
        let mut s = seed;
        let mut next = move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 40) as f32 / (1u64 << 24) as f32 - 0.5
        };
        (0..n).map(|_| (0..dim).map(|_| next()).collect()).collect()
    }

    fn small() -> Index {
        let i = Index::temp(&Config::new(2, Metric::Cosine)).unwrap();
        i.insert(
            10,
            &[1.0, 0.0],
            meta(&[("kind", "kb"), ("lang", "de")]),
            Some(b"ten".to_vec()),
        )
        .unwrap();
        i.insert(
            20,
            &[0.0, 1.0],
            meta(&[("kind", "ticket"), ("lang", "en")]),
            None,
        )
        .unwrap();
        i.insert(
            30,
            &[1.0, 1.0],
            meta(&[("kind", "kb"), ("lang", "en")]),
            None,
        )
        .unwrap();
        i
    }

    #[test]
    fn exact_and_hnsw_order_by_distance() {
        let i = small();
        for exact in [true, false] {
            let hits = i.search(&[1.0, 0.1], &vec![], &opts(3, exact, 0)).unwrap();
            assert_eq!(ids(&hits), vec![10, 30, 20]);
            assert!(hits[0].1 < hits[1].1 && hits[1].1 < hits[2].1);
        }
    }

    #[test]
    fn filters_and_across_keys_or_within_key() {
        let i = small();
        for threshold in [0, 4096] {
            let o = opts(3, false, threshold);
            assert_eq!(
                ids(&i
                    .search(&[1.0, 0.0], &filter(&[("kind", &["kb"])]), &o)
                    .unwrap()),
                vec![10, 30]
            );
            let f = filter(&[("kind", &["kb", "ticket"]), ("lang", &["en"])]);
            assert_eq!(ids(&i.search(&[0.0, 1.0], &f, &o).unwrap()), vec![20, 30]);
            assert!(i
                .search(&[1.0, 0.0], &filter(&[("kind", &["nope"])]), &o)
                .unwrap()
                .is_empty());
        }
    }

    #[test]
    fn payloads_delete_and_upsert() {
        let i = small();
        let with_payload = SearchOpts {
            payload: true,
            ..opts(1, true, 0)
        };
        assert_eq!(
            i.search(&[1.0, 0.0], &vec![], &with_payload).unwrap()[0]
                .2
                .as_deref(),
            Some(&b"ten"[..])
        );
        assert!(i.delete(10).unwrap());
        assert!(!i.delete(10).unwrap());
        assert_eq!(i.get_payload(10).unwrap(), None);
        for exact in [true, false] {
            assert_eq!(
                ids(&i.search(&[1.0, 0.0], &vec![], &opts(3, exact, 0)).unwrap()),
                vec![30, 20]
            );
        }
        assert!(i
            .search(
                &[1.0, 0.0],
                &filter(&[("lang", &["de"])]),
                &opts(3, false, 0)
            )
            .unwrap()
            .is_empty());
        i.insert(
            20,
            &[1.0, 0.0],
            meta(&[("kind", "kb")]),
            Some(b"new".to_vec()),
        )
        .unwrap();
        assert_eq!(
            ids(&i
                .search(
                    &[1.0, 0.0],
                    &filter(&[("kind", &["kb"])]),
                    &opts(1, false, 0)
                )
                .unwrap()),
            vec![20]
        );
        assert!(i
            .search(
                &[1.0, 0.0],
                &filter(&[("kind", &["ticket"])]),
                &opts(3, false, 0)
            )
            .unwrap()
            .is_empty());
        assert_eq!(i.get_payload(20).unwrap().as_deref(), Some(&b"new"[..]));
        let (_, _, live, deleted, _, _) = i.info().unwrap();
        assert_eq!((live, deleted), (2, 2));
    }

    #[test]
    fn insert_many_is_all_or_nothing() {
        let i = small();
        let bad = vec![
            (1, vec![1.0, 0.0], vec![], None),
            (2, vec![0.0, 0.0], vec![], None),
        ];
        let err = i.insert_many(bad).unwrap_err();
        assert_eq!(
            (err.reason, err.message.starts_with("item 1")),
            ("invalid_vector", true)
        );
        assert_eq!(i.len(), 3);
        assert_eq!(
            i.insert(1, &[1.0], vec![], None).unwrap_err().reason,
            "dimension_mismatch"
        );
    }

    #[test]
    fn reopen_persists_everything_and_validates_config() {
        let dir = std::env::temp_dir().join(format!("bi-reopen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cfg = Config::new(8, Metric::Cosine);
        let vs = random_vectors(300, 8, 3);
        let f = filter(&[("p", &["even"])]);
        let o = SearchOpts {
            payload: true,
            ..opts(10, false, 0)
        };
        let before = {
            let i = Index::open(&dir, &cfg).unwrap();
            let items = vs
                .iter()
                .enumerate()
                .map(|(n, v)| {
                    (
                        n as u64,
                        v.clone(),
                        meta(&[("p", if n % 2 == 0 { "even" } else { "odd" })]),
                        Some(vec![n as u8]),
                    )
                })
                .collect();
            i.insert_many(items).unwrap();
            i.delete(4).unwrap();
            i.search(&vs[5], &f, &o).unwrap()
        };
        let i = Index::open(&dir, &cfg).unwrap();
        assert_eq!(i.len(), 299);
        let after = i.search(&vs[5], &f, &o).unwrap();
        assert_eq!(before, after);
        assert!(!ids(&after).contains(&4));
        let bdir = dir.join("backup");
        i.backup(&bdir).unwrap();
        drop(i);
        assert_eq!(Index::open(&bdir, &cfg).unwrap().len(), 299);
        assert_eq!(
            Index::open(&dir, &Config::new(9, Metric::Cosine))
                .err()
                .unwrap()
                .reason,
            "dimension_mismatch"
        );
        assert_eq!(
            Index::open(&dir, &Config::new(8, Metric::Lorentz))
                .err()
                .unwrap()
                .reason,
            "invalid_argument"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn temp_index_removes_its_directory() {
        let i = Index::temp(&Config::new(2, Metric::Cosine)).unwrap();
        let dir = i._temp.as_ref().unwrap().0.clone();
        assert!(dir.exists());
        drop(i);
        assert!(!dir.exists());
    }

    fn recall(i: &Index, queries: &[Vec<f32>], f: &Filter, o: &SearchOpts) -> f64 {
        let mut hit = 0;
        for q in queries {
            let exact = ids(&i
                .search(
                    q,
                    f,
                    &SearchOpts {
                        exact: true,
                        ..o.clone()
                    },
                )
                .unwrap());
            let approx = ids(&i.search(q, f, o).unwrap());
            hit += approx.iter().filter(|x| exact.contains(x)).count();
        }
        hit as f64 / (queries.len() * o.k) as f64
    }

    #[test]
    fn hnsw_recall_plain_and_filtered() {
        let i = Index::temp(&Config::new(32, Metric::Cosine)).unwrap();
        let items = random_vectors(3000, 32, 7)
            .into_iter()
            .enumerate()
            .map(|(n, v)| {
                (
                    n as u64,
                    v,
                    meta(&[("tag", if n % 10 == 0 { "rare" } else { "common" })]),
                    None,
                )
            })
            .collect();
        i.insert_many(items).unwrap();
        let qs = random_vectors(50, 32, 99);
        assert!(recall(&i, &qs, &vec![], &opts(10, false, 0)) >= 0.95);
        let forced_hnsw = opts(10, false, 0);
        assert!(recall(&i, &qs, &filter(&[("tag", &["rare"])]), &forced_hnsw) >= 0.95);
        let hits = i
            .search(&qs[0], &filter(&[("tag", &["rare"])]), &forced_hnsw)
            .unwrap();
        assert!(hits.iter().all(|h| h.0 % 10 == 0));
    }

    #[test]
    fn deterministic_graph() {
        let build = || {
            let i = Index::temp(&Config::new(8, Metric::Cosine)).unwrap();
            let items = random_vectors(200, 8, 3)
                .into_iter()
                .enumerate()
                .map(|(n, v)| (n as u64, v, vec![], None))
                .collect();
            i.insert_many(items).unwrap();
            i.search(&[0.1; 8], &vec![], &opts(20, false, 0)).unwrap()
        };
        assert_eq!(build(), build());
    }

    fn load_fixture(name: &str, dim: usize) -> Vec<Vec<f32>> {
        let path = format!("{}/../../test/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        std::fs::read(path)
            .unwrap()
            .chunks_exact(dim * 4)
            .map(crate::store::decode_f32s)
            .collect()
    }

    #[test]
    fn fixtures_hnsw_matches_exact_at_default_ef() {
        for (model, dim, metric) in [("e5", 768, Metric::Cosine), ("v5", 129, Metric::Lorentz)] {
            let i = Index::temp(&Config::new(dim, metric)).unwrap();
            let items = load_fixture(&format!("{model}_chunks.f32"), dim)
                .into_iter()
                .enumerate()
                .map(|(n, v)| (n as u64, v, vec![], None))
                .collect();
            i.insert_many(items).unwrap();
            let qs = load_fixture(&format!("{model}_queries.f32"), dim);
            let o = SearchOpts {
                k: 10,
                ef: 100,
                ..Default::default()
            };
            assert!(recall(&i, &qs, &vec![], &o) >= 0.98, "{model}");
        }
    }
}
