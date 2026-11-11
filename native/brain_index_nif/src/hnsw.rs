//! HNSW graph (Malkov & Yashunin, 2016). Structure ported from HyperspaceDB `hyperspace-index`
//! (MIT, YARlabs): greedy upper-layer descent, layer-0 beam search with an epoch-visited scratch,
//! heuristic neighbour selection and bidirectional link pruning. Quantization, MRL, wave search
//! and environment-driven tuning were deliberately left out (ADR002).

use crate::index::Index;
use crate::topk::Scored;
use std::cell::RefCell;
use std::collections::BinaryHeap;

pub const MAX_LEVEL: usize = 16;

#[derive(Debug, Clone)]
pub struct Params {
    /// Max links per node on layers >= 1; layer 0 allows `2 * m`.
    pub m: usize,
    pub ef_construction: usize,
    pub seed: u64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            m: 16,
            ef_construction: 200,
            seed: 0x5EED_B2A1_1DE7,
        }
    }
}

pub struct Graph {
    pub params: Params,
    /// links[node][level] = neighbour internal ids.
    pub links: Vec<Vec<Vec<u32>>>,
    pub entry: Option<u32>,
    pub max_level: usize,
    pub rng: u64,
}

impl Graph {
    pub fn new(params: Params) -> Self {
        let rng = params.seed.max(1);
        Self {
            params,
            links: Vec::new(),
            entry: None,
            max_level: 0,
            rng,
        }
    }

    fn max_links(&self, level: usize) -> usize {
        if level == 0 {
            self.params.m * 2
        } else {
            self.params.m
        }
    }

    /// Geometric level distribution with mL = 1/ln(M), from a deterministic xorshift64.
    fn random_level(&mut self) -> usize {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        let u = ((self.rng >> 11) as f64 + 1.0) / (1u64 << 53) as f64;
        let ml = 1.0 / (self.params.m as f64).ln();
        ((-u.ln() * ml) as usize).min(MAX_LEVEL)
    }
}

thread_local! {
    /// Per-thread visited marks: node is visited iff marks[node] == epoch.
    static VISITED: RefCell<(Vec<u32>, u32)> = const { RefCell::new((Vec::new(), 0)) };
}

/// Runs `f` with a fresh visited-set over `n` nodes. Allocation is reused across calls.
fn with_visited<R>(n: usize, f: impl FnOnce(&mut dyn FnMut(u32) -> bool) -> R) -> R {
    VISITED.with(|cell| {
        let mut guard = cell.borrow_mut();
        let (marks, epoch) = &mut *guard;
        if marks.len() < n {
            marks.resize(n, 0);
        }
        *epoch = epoch.wrapping_add(1);
        if *epoch == 0 {
            marks.iter_mut().for_each(|m| *m = 0);
            *epoch = 1;
        }
        let e = *epoch;
        let mut first_visit = |node: u32| {
            let slot = &mut marks[node as usize];
            let fresh = *slot != e;
            *slot = e;
            fresh
        };
        f(&mut first_visit)
    })
}

/// Min-heap adapter.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Near(std::cmp::Reverse<Scored>);

impl Index {
    #[inline]
    fn dist_to(&self, q: &[f32], node: u32) -> f32 {
        self.metric.distance(q, self.vector(node))
    }

    /// Greedy walk to the closest node on `level`, starting at `ep`.
    fn greedy(&self, q: &[f32], mut ep: Scored, level: usize) -> Scored {
        loop {
            let mut improved = false;
            for &n in &self.graph.links[ep.id as usize][level] {
                let d = self.dist_to(q, n);
                if d < ep.dist {
                    ep = Scored { dist: d, id: n };
                    improved = true;
                }
            }
            if !improved {
                return ep;
            }
        }
    }

    /// Beam search on one layer. All nodes navigate; only nodes passing `accept` enter results.
    /// Returns up to `ef` accepted nodes sorted ascending.
    pub(crate) fn search_layer(
        &self,
        q: &[f32],
        ep: Scored,
        ef: usize,
        level: usize,
        accept: &dyn Fn(u32) -> bool,
    ) -> Vec<Scored> {
        with_visited(self.ext_ids.len(), |first_visit| {
            let mut candidates: BinaryHeap<Near> = BinaryHeap::new();
            let mut results: BinaryHeap<Scored> = BinaryHeap::new();
            first_visit(ep.id);
            candidates.push(Near(std::cmp::Reverse(ep)));
            if accept(ep.id) {
                results.push(ep);
            }
            while let Some(Near(std::cmp::Reverse(c))) = candidates.pop() {
                if results.len() >= ef && c.dist > results.peek().unwrap().dist {
                    break;
                }
                for &n in &self.graph.links[c.id as usize][level] {
                    if !first_visit(n) {
                        continue;
                    }
                    let d = self.dist_to(q, n);
                    let full = results.len() >= ef;
                    if !full || d < results.peek().unwrap().dist {
                        let s = Scored { dist: d, id: n };
                        candidates.push(Near(std::cmp::Reverse(s)));
                        if accept(n) {
                            results.push(s);
                            if results.len() > ef {
                                results.pop();
                            }
                        }
                    }
                }
            }
            results.into_sorted_vec()
        })
    }

    /// Heuristic selection (Malkov alg. 4): prefer candidates closer to the base than to any
    /// already-selected neighbour, then fill remaining slots with the closest pruned ones.
    fn select_neighbors(&self, sorted: &[Scored], m: usize) -> Vec<u32> {
        let mut chosen: Vec<Scored> = Vec::with_capacity(m);
        let mut pruned: Vec<u32> = Vec::new();
        for &c in sorted {
            if chosen.len() >= m {
                break;
            }
            let cv = self.vector(c.id);
            if chosen
                .iter()
                .all(|s| self.metric.distance(cv, self.vector(s.id)) > c.dist)
            {
                chosen.push(c);
            } else {
                pruned.push(c.id);
            }
        }
        let mut out: Vec<u32> = chosen.iter().map(|s| s.id).collect();
        out.extend(pruned.into_iter().take(m.saturating_sub(out.len())));
        out
    }

    /// Links an already-stored node into the graph.
    pub(crate) fn graph_insert(&mut self, node: u32) {
        let level = self.graph.random_level();
        self.graph.links.push(vec![Vec::new(); level + 1]);
        let Some(entry) = self.graph.entry else {
            self.graph.entry = Some(node);
            self.graph.max_level = level;
            return;
        };
        let q = self.vector(node).to_vec();
        let mut ep = Scored {
            dist: self.dist_to(&q, entry),
            id: entry,
        };
        for l in (level + 1..=self.graph.max_level).rev() {
            ep = self.greedy(&q, ep, l);
        }
        let ef = self.graph.params.ef_construction;
        for l in (0..=level.min(self.graph.max_level)).rev() {
            let found = self.search_layer(&q, ep, ef, l, &|_| true);
            let max = self.graph.max_links(l);
            let neighbors = self.select_neighbors(&found, self.graph.params.m);
            for &n in &neighbors {
                self.link(n, node, l, max);
            }
            self.graph.links[node as usize][l] = neighbors;
            ep = found[0];
        }
        if level > self.graph.max_level {
            self.graph.max_level = level;
            self.graph.entry = Some(node);
        }
    }

    /// Adds `to` to `from`'s links on `level`, pruning with the heuristic when over capacity.
    fn link(&mut self, from: u32, to: u32, level: usize, max: usize) {
        self.graph.links[from as usize][level].push(to);
        if self.graph.links[from as usize][level].len() <= max {
            return;
        }
        let base = self.vector(from).to_vec();
        let mut scored: Vec<Scored> = self.graph.links[from as usize][level]
            .iter()
            .map(|&n| Scored {
                dist: self.dist_to(&base, n),
                id: n,
            })
            .collect();
        scored.sort();
        self.graph.links[from as usize][level] = self.select_neighbors(&scored, max);
    }

    /// Approximate k-NN over nodes passing `accept`.
    pub(crate) fn hnsw_search(
        &self,
        q: &[f32],
        k: usize,
        ef: usize,
        accept: &dyn Fn(u32) -> bool,
    ) -> Vec<Scored> {
        let Some(entry) = self.graph.entry else {
            return Vec::new();
        };
        let mut ep = Scored {
            dist: self.dist_to(q, entry),
            id: entry,
        };
        for l in (1..=self.graph.max_level).rev() {
            ep = self.greedy(q, ep, l);
        }
        let mut hits = self.search_layer(q, ep, ef.max(k), 0, accept);
        hits.truncate(k);
        hits
    }
}

#[cfg(test)]
mod tests {
    use crate::index::{Index, SearchOpts};
    use crate::metric::Metric;

    fn opts(k: usize, ef: usize, exact: bool) -> SearchOpts {
        SearchOpts {
            k,
            ef,
            exact,
            ..Default::default()
        }
    }

    fn random_vectors(n: usize, dim: usize, seed: u64) -> Vec<Vec<f32>> {
        let mut s = seed;
        let mut next = move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 40) as f32 / (1u64 << 24) as f32 - 0.5
        };
        (0..n).map(|_| (0..dim).map(|_| next()).collect()).collect()
    }

    fn recall_vs_exact(idx: &Index, queries: &[Vec<f32>], k: usize) -> f64 {
        let mut hit = 0;
        for q in queries {
            let exact: Vec<u64> = idx
                .search(q, &vec![], &opts(k, 0, true))
                .unwrap()
                .iter()
                .map(|h| h.0)
                .collect();
            let approx: Vec<u64> = idx
                .search(q, &vec![], &opts(k, 64, false))
                .unwrap()
                .iter()
                .map(|h| h.0)
                .collect();
            hit += approx.iter().filter(|id| exact.contains(id)).count();
        }
        hit as f64 / (queries.len() * k) as f64
    }

    #[test]
    fn hnsw_recall_vs_exact_on_random_data() {
        let mut idx = Index::new(32, Metric::Cosine).unwrap();
        for (i, v) in random_vectors(3000, 32, 7).iter().enumerate() {
            idx.insert(i as u64, v, vec![]).unwrap();
        }
        let r = recall_vs_exact(&idx, &random_vectors(100, 32, 99), 10);
        assert!(r >= 0.95, "recall {r}");
    }

    #[test]
    fn filtered_hnsw_matches_filtered_exact() {
        let mut idx = Index::new(32, Metric::Cosine).unwrap();
        for (i, v) in random_vectors(3000, 32, 11).iter().enumerate() {
            let tag = if i % 10 == 0 { "rare" } else { "common" };
            idx.insert(i as u64, v, vec![("tag".into(), tag.into())])
                .unwrap();
        }
        let f = vec![("tag".to_string(), vec!["rare".to_string()])];
        let forced_hnsw = SearchOpts {
            k: 10,
            ef: 100,
            exact: false,
            brute_force_threshold: 0,
        };
        let (mut hit, mut total) = (0, 0);
        for q in random_vectors(50, 32, 5) {
            let exact: Vec<u64> = idx
                .search(&q, &f, &opts(10, 0, true))
                .unwrap()
                .iter()
                .map(|h| h.0)
                .collect();
            let approx = idx.search(&q, &f, &forced_hnsw).unwrap();
            assert!(approx.iter().all(|h| h.0 % 10 == 0), "filter violated");
            hit += approx.iter().filter(|h| exact.contains(&h.0)).count();
            total += 10;
        }
        assert!(
            hit as f64 / total as f64 >= 0.95,
            "recall {}",
            hit as f64 / total as f64
        );
    }

    #[test]
    fn deterministic_graph() {
        let build = || {
            let mut idx = Index::new(8, Metric::Cosine).unwrap();
            for (i, v) in random_vectors(300, 8, 3).iter().enumerate() {
                idx.insert(i as u64, v, vec![]).unwrap();
            }
            idx.graph.links.clone()
        };
        assert_eq!(build(), build());
    }

    #[test]
    fn empty_and_single_node() {
        let mut idx = Index::new(2, Metric::Cosine).unwrap();
        assert!(idx
            .search(&[1.0, 0.0], &vec![], &opts(3, 10, false))
            .unwrap()
            .is_empty());
        idx.insert(5, &[1.0, 0.0], vec![]).unwrap();
        assert_eq!(
            idx.search(&[0.0, 1.0], &vec![], &opts(3, 10, false))
                .unwrap()[0]
                .0,
            5
        );
    }
}

#[cfg(test)]
mod fixture_tests {
    use crate::index::Index;
    use crate::metric::Metric;

    pub fn load(name: &str, dim: usize) -> Vec<Vec<f32>> {
        let path = format!("{}/../../test/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        let bytes = std::fs::read(path).unwrap();
        bytes
            .chunks_exact(dim * 4)
            .map(|c| {
                c.chunks_exact(4)
                    .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                    .collect()
            })
            .collect()
    }

    fn reachable(idx: &Index) -> usize {
        let n = idx.ext_ids.len();
        let mut seen = vec![false; n];
        let mut stack = vec![idx.graph.entry.unwrap()];
        while let Some(x) = stack.pop() {
            if std::mem::replace(&mut seen[x as usize], true) {
                continue;
            }
            stack.extend(idx.graph.links[x as usize][0].iter().copied());
        }
        seen.iter().filter(|s| **s).count()
    }

    #[test]
    fn every_node_reachable_on_layer0() {
        for (file, dim, metric) in [
            ("e5_chunks.f32", 768, Metric::Cosine),
            ("v5_chunks.f32", 129, Metric::Lorentz),
        ] {
            let mut idx = Index::new(dim, metric).unwrap();
            for (i, v) in load(file, dim).iter().enumerate() {
                idx.insert(i as u64, v, vec![]).unwrap();
            }
            assert_eq!(reachable(&idx), idx.len(), "{file}");
        }
    }
}
