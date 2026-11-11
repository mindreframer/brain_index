//! The index: vector arena, id mapping, metadata postings and search entry points.

use crate::bitset::Bitset;
use crate::error::{Error, Result};
use crate::hnsw::{Graph, Params};
use crate::metric::Metric;
use crate::topk::{Scored, TopK};
use std::collections::HashMap;

/// Metadata of one vector: exact-match string pairs (ADR004).
pub type Meta = Vec<(String, String)>;

/// Filter: AND across keys, OR across the values of one key.
pub type Filter = Vec<(String, Vec<String>)>;

#[derive(Debug, Clone)]
pub struct SearchOpts {
    pub k: usize,
    pub ef: usize,
    pub exact: bool,
    /// Filtered searches whose allowed set is at most this large run as exact brute force.
    pub brute_force_threshold: usize,
}

impl Default for SearchOpts {
    fn default() -> Self {
        Self {
            k: 10,
            ef: 100,
            exact: false,
            brute_force_threshold: 4096,
        }
    }
}

pub struct Index {
    pub(crate) dim: usize,
    pub(crate) metric: Metric,
    /// Contiguous f32 arena; node `i` occupies `[i*dim, (i+1)*dim)`.
    pub(crate) vectors: Vec<f32>,
    /// Internal id -> external id.
    pub(crate) ext_ids: Vec<u64>,
    /// External id -> internal id (live nodes only).
    pub(crate) by_ext: HashMap<u64, u32>,
    /// Internal id -> metadata.
    pub(crate) node_meta: Vec<Meta>,
    /// key -> value -> nodes carrying that pair.
    pub(crate) postings: HashMap<String, HashMap<String, Bitset>>,
    pub(crate) graph: Graph,
}

impl Index {
    pub fn new(dim: usize, metric: Metric) -> Result<Self> {
        Self::with_params(dim, metric, Params::default())
    }

    pub fn with_params(dim: usize, metric: Metric, params: Params) -> Result<Self> {
        if dim == 0 || dim > 65_536 {
            return Err(Error::invalid_argument("dim must be in 1..=65536"));
        }
        if !(2..=256).contains(&params.m) || params.ef_construction < 1 {
            return Err(Error::invalid_argument(
                "m must be in 2..=256 and ef_construction >= 1",
            ));
        }
        Ok(Self {
            dim,
            metric,
            vectors: Vec::new(),
            ext_ids: Vec::new(),
            by_ext: HashMap::new(),
            node_meta: Vec::new(),
            postings: HashMap::new(),
            graph: Graph::new(params),
        })
    }

    pub fn len(&self) -> usize {
        self.by_ext.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_ext.is_empty()
    }

    #[inline]
    pub(crate) fn vector(&self, node: u32) -> &[f32] {
        let s = node as usize * self.dim;
        &self.vectors[s..s + self.dim]
    }

    /// Validates, canonicalises and returns an owned copy of `v`.
    pub(crate) fn prepare(&self, v: &[f32]) -> Result<Vec<f32>> {
        if v.len() != self.dim {
            return Err(Error::dimension_mismatch(self.dim, v.len()));
        }
        let mut v = v.to_vec();
        self.metric.prepare(&mut v)?;
        Ok(v)
    }

    pub fn insert(&mut self, ext_id: u64, v: &[f32], meta: Meta) -> Result<()> {
        let v = self.prepare(v)?;
        if self.ext_ids.len() >= u32::MAX as usize {
            return Err(Error::invalid_argument("index is full"));
        }
        let node = self.ext_ids.len() as u32;
        self.vectors.extend_from_slice(&v);
        self.ext_ids.push(ext_id);
        self.by_ext.insert(ext_id, node);
        for (k, val) in &meta {
            self.postings
                .entry(k.clone())
                .or_default()
                .entry(val.clone())
                .or_default()
                .set(node);
        }
        self.node_meta.push(meta);
        self.graph_insert(node);
        Ok(())
    }

    /// Allowed nodes for `filter`, or `None` when the filter is empty (everything allowed).
    fn compile_filter(&self, filter: &Filter) -> Option<Bitset> {
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
        acc
    }

    #[inline]
    fn is_live(&self, node: u32) -> bool {
        self.by_ext.get(&self.ext_ids[node as usize]) == Some(&node)
    }

    /// Unified search: exact or HNSW, optionally filtered (ADR004 strategy).
    pub fn search(&self, q: &[f32], filter: &Filter, o: &SearchOpts) -> Result<Vec<(u64, f32)>> {
        let q = self.prepare(q)?;
        let allowed = self.compile_filter(filter);
        let hits = match &allowed {
            Some(bits) if o.exact || bits.count() <= o.brute_force_threshold => {
                self.brute_force(&q, o.k, bits.iter())
            }
            Some(bits) => self.hnsw_search(&q, o.k, o.ef, &|n| bits.contains(n) && self.is_live(n)),
            None if o.exact => self.brute_force(&q, o.k, self.by_ext.values().copied()),
            None => self.hnsw_search(&q, o.k, o.ef, &|n| self.is_live(n)),
        };
        Ok(self.to_external(hits))
    }

    fn brute_force(&self, q: &[f32], k: usize, nodes: impl Iterator<Item = u32>) -> Vec<Scored> {
        let mut top = TopK::new(k);
        for node in nodes.filter(|&n| self.is_live(n)) {
            top.push(Scored {
                dist: self.metric.distance(q, self.vector(node)),
                id: node,
            });
        }
        top.into_sorted()
    }

    pub(crate) fn to_external(&self, hits: Vec<Scored>) -> Vec<(u64, f32)> {
        hits.into_iter()
            .map(|s| (self.ext_ids[s.id as usize], s.dist))
            .collect()
    }
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

    pub fn ids(hits: &[(u64, f32)]) -> Vec<u64> {
        hits.iter().map(|h| h.0).collect()
    }

    fn idx() -> Index {
        let mut i = Index::new(2, Metric::Cosine).unwrap();
        i.insert(10, &[1.0, 0.0], meta(&[("kind", "kb"), ("lang", "de")]))
            .unwrap();
        i.insert(20, &[0.0, 1.0], meta(&[("kind", "ticket"), ("lang", "en")]))
            .unwrap();
        i.insert(30, &[1.0, 1.0], meta(&[("kind", "kb"), ("lang", "en")]))
            .unwrap();
        i
    }

    fn opts(k: usize, exact: bool, threshold: usize) -> SearchOpts {
        SearchOpts {
            k,
            ef: 10,
            exact,
            brute_force_threshold: threshold,
        }
    }

    #[test]
    fn exact_search_orders_by_distance() {
        let hits = idx()
            .search(&[1.0, 0.1], &vec![], &opts(3, true, 0))
            .unwrap();
        assert_eq!(ids(&hits), vec![10, 30, 20]);
        assert!(hits[0].1 < hits[1].1 && hits[1].1 < hits[2].1);
    }

    #[test]
    fn filters_and_across_keys_or_within_key() {
        let i = idx();
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
            assert!(i
                .search(&[1.0, 0.0], &filter(&[("missing", &["x"])]), &o)
                .unwrap()
                .is_empty());
        }
    }

    #[test]
    fn rejects_wrong_dimension() {
        let mut i = idx();
        assert_eq!(
            i.insert(1, &[1.0], vec![]).unwrap_err().reason,
            "dimension_mismatch"
        );
        let err = i
            .search(&[1.0, 2.0, 3.0], &vec![], &opts(1, true, 0))
            .unwrap_err();
        assert_eq!(err.reason, "dimension_mismatch");
    }
}
