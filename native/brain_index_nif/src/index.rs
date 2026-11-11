//! The index: vector arena, id mapping and search entry points.

use crate::error::{Error, Result};
use crate::hnsw::{Graph, Params};
use crate::metric::Metric;
use crate::topk::{Scored, TopK};
use std::collections::HashMap;

pub struct Index {
    pub(crate) dim: usize,
    pub(crate) metric: Metric,
    /// Contiguous f32 arena; node `i` occupies `[i*dim, (i+1)*dim)`.
    pub(crate) vectors: Vec<f32>,
    /// Internal id -> external id.
    pub(crate) ext_ids: Vec<u64>,
    /// External id -> internal id (live nodes only).
    pub(crate) by_ext: HashMap<u64, u32>,
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

    pub fn insert(&mut self, ext_id: u64, v: &[f32]) -> Result<()> {
        let v = self.prepare(v)?;
        if self.ext_ids.len() >= u32::MAX as usize {
            return Err(Error::invalid_argument("index is full"));
        }
        let node = self.ext_ids.len() as u32;
        self.vectors.extend_from_slice(&v);
        self.ext_ids.push(ext_id);
        self.by_ext.insert(ext_id, node);
        self.graph_insert(node);
        Ok(())
    }

    /// Approximate k-NN via HNSW.
    pub fn search(&self, q: &[f32], k: usize, ef: usize) -> Result<Vec<(u64, f32)>> {
        let q = self.prepare(q)?;
        Ok(self.to_external(self.hnsw_search(&q, k, ef, &|_| true)))
    }

    /// Exact search over all live nodes accepted by `allow`.
    pub fn exact_search(
        &self,
        q: &[f32],
        k: usize,
        allow: impl Fn(u32) -> bool,
    ) -> Result<Vec<(u64, f32)>> {
        let q = self.prepare(q)?;
        let mut top = TopK::new(k);
        for &node in self.by_ext.values() {
            if allow(node) {
                top.push(Scored {
                    dist: self.metric.distance(&q, self.vector(node)),
                    id: node,
                });
            }
        }
        Ok(self.to_external(top.into_sorted()))
    }

    pub(crate) fn to_external(&self, hits: Vec<Scored>) -> Vec<(u64, f32)> {
        hits.into_iter()
            .map(|s| (self.ext_ids[s.id as usize], s.dist))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idx() -> Index {
        let mut i = Index::new(2, Metric::Cosine).unwrap();
        i.insert(10, &[1.0, 0.0]).unwrap();
        i.insert(20, &[0.0, 1.0]).unwrap();
        i.insert(30, &[1.0, 1.0]).unwrap();
        i
    }

    #[test]
    fn exact_search_orders_by_distance() {
        let hits = idx().exact_search(&[1.0, 0.1], 3, |_| true).unwrap();
        let ids: Vec<u64> = hits.iter().map(|h| h.0).collect();
        assert_eq!(ids, vec![10, 30, 20]);
        assert!(hits[0].1 < hits[1].1 && hits[1].1 < hits[2].1);
    }

    #[test]
    fn rejects_wrong_dimension() {
        let mut i = idx();
        assert_eq!(
            i.insert(1, &[1.0]).unwrap_err().reason,
            "dimension_mismatch"
        );
        assert_eq!(
            i.exact_search(&[1.0, 2.0, 3.0], 1, |_| true)
                .unwrap_err()
                .reason,
            "dimension_mismatch"
        );
    }
}
