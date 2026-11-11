//! HNSW graph (Malkov & Yashunin, 2016) over LMDB transactions (ADR009). Structure ported from
//! HyperspaceDB `hyperspace-index` (MIT, YARlabs): greedy upper-layer descent, layer-0 beam search
//! with an epoch-visited scratch, heuristic neighbour selection, bidirectional link pruning.
//! Reads go through a `View` (any transaction) straight from the memory map; writes take `RwTxn`.

use crate::error::Result;
use crate::metric::Metric;
use crate::store::{decode_f32s, link_key, node_key, Header, Store};
use crate::topk::Scored;
use heed::{RoTxn, RwTxn};
use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

pub const MAX_LEVEL: usize = 16;

/// Read access to vectors and links inside one transaction (read or write).
pub struct View<'a, 'e> {
    pub txn: &'a RoTxn<'e>,
    pub s: &'a Store,
    pub metric: Metric,
}

impl<'a> View<'a, '_> {
    #[inline]
    pub fn vector(&self, n: u32) -> Result<&'a [u8]> {
        Ok(self.s.vectors.get(self.txn, &node_key(n))?.unwrap_or(&[]))
    }

    #[inline]
    pub fn links(&self, n: u32, level: usize) -> Result<&'a [u8]> {
        Ok(self
            .s
            .links
            .get(self.txn, &link_key(n, level as u8))?
            .unwrap_or(&[]))
    }

    #[inline]
    pub fn dist(&self, q: &[f32], n: u32) -> Result<f32> {
        Ok(self.metric.distance_bytes(q, self.vector(n)?))
    }
}

#[inline]
fn ids(links: &[u8]) -> impl Iterator<Item = u32> + '_ {
    links
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
}

fn encode_ids(ids: &[u32]) -> Vec<u8> {
    ids.iter().flat_map(|i| i.to_le_bytes()).collect()
}

thread_local! {
    /// Per-thread visited marks: node is visited iff marks[node] == epoch.
    static VISITED: RefCell<(Vec<u32>, u32)> = const { RefCell::new((Vec::new(), 0)) };
}

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

/// Deterministic level from the persisted xorshift64 state (mL = 1/ln M).
pub fn random_level(h: &mut Header) -> usize {
    h.rng ^= h.rng << 13;
    h.rng ^= h.rng >> 7;
    h.rng ^= h.rng << 17;
    let u = ((h.rng >> 11) as f64 + 1.0) / (1u64 << 53) as f64;
    ((-u.ln() / (h.m as f64).ln()) as usize).min(MAX_LEVEL)
}

fn greedy(v: &View, q: &[f32], mut ep: Scored, level: usize) -> Result<Scored> {
    loop {
        let mut improved = false;
        for n in ids(v.links(ep.id, level)?) {
            let d = v.dist(q, n)?;
            if d < ep.dist {
                ep = Scored { dist: d, id: n };
                improved = true;
            }
        }
        if !improved {
            return Ok(ep);
        }
    }
}

/// Beam search on one layer. All nodes navigate; only nodes passing `accept` enter results.
pub fn search_layer(
    v: &View,
    n_nodes: usize,
    q: &[f32],
    ep: Scored,
    ef: usize,
    level: usize,
    accept: &dyn Fn(u32) -> bool,
) -> Result<Vec<Scored>> {
    with_visited(n_nodes, |first_visit| {
        let mut candidates: BinaryHeap<Reverse<Scored>> = BinaryHeap::new();
        let mut results: BinaryHeap<Scored> = BinaryHeap::new();
        first_visit(ep.id);
        candidates.push(Reverse(ep));
        if accept(ep.id) {
            results.push(ep);
        }
        while let Some(Reverse(c)) = candidates.pop() {
            if results.len() >= ef && c.dist > results.peek().unwrap().dist {
                break;
            }
            for n in ids(v.links(c.id, level)?) {
                if !first_visit(n) {
                    continue;
                }
                let d = v.dist(q, n)?;
                if results.len() < ef || d < results.peek().unwrap().dist {
                    let s = Scored { dist: d, id: n };
                    candidates.push(Reverse(s));
                    if accept(n) {
                        results.push(s);
                        if results.len() > ef {
                            results.pop();
                        }
                    }
                }
            }
        }
        Ok(results.into_sorted_vec())
    })
}

/// Heuristic selection (Malkov alg. 4) with fill from pruned candidates.
fn select_neighbors(v: &View, sorted: &[Scored], m: usize) -> Result<Vec<u32>> {
    let mut chosen: Vec<(Scored, Vec<f32>)> = Vec::with_capacity(m);
    let mut pruned = Vec::new();
    for &c in sorted {
        if chosen.len() >= m {
            break;
        }
        let cv = decode_f32s(v.vector(c.id)?);
        if chosen
            .iter()
            .all(|(_, sv)| v.metric.distance(&cv, sv) > c.dist)
        {
            chosen.push((c, cv));
        } else {
            pruned.push(c.id);
        }
    }
    let mut out: Vec<u32> = chosen.iter().map(|(s, _)| s.id).collect();
    out.extend(pruned.into_iter().take(m.saturating_sub(out.len())));
    Ok(out)
}

/// Approximate k-NN over nodes passing `accept`, in one read view.
pub fn search(
    v: &View,
    h: &Header,
    q: &[f32],
    k: usize,
    ef: usize,
    accept: &dyn Fn(u32) -> bool,
) -> Result<Vec<Scored>> {
    let Some(entry) = h.entry else {
        return Ok(Vec::new());
    };
    let mut ep = Scored {
        dist: v.dist(q, entry)?,
        id: entry,
    };
    for l in (1..=h.max_level).rev() {
        ep = greedy(v, q, ep, l)?;
    }
    let mut hits = search_layer(v, h.next_node as usize, q, ep, ef.max(k), 0, accept)?;
    hits.truncate(k);
    Ok(hits)
}

/// Links the freshly stored `node` (vector already written) into the graph. Updates `h`.
pub fn insert(
    w: &mut RwTxn,
    s: &Store,
    h: &mut Header,
    node: u32,
    q: &[f32],
    level: usize,
) -> Result<()> {
    let metric = h.metric;
    let Some(entry) = h.entry else {
        h.entry = Some(node);
        h.max_level = level;
        return Ok(());
    };
    let mut ep = {
        let v = View { txn: w, s, metric };
        let mut ep = Scored {
            dist: v.dist(q, entry)?,
            id: entry,
        };
        for l in (level + 1..=h.max_level).rev() {
            ep = greedy(&v, q, ep, l)?;
        }
        ep
    };
    for l in (0..=level.min(h.max_level)).rev() {
        let max = if l == 0 { h.m * 2 } else { h.m };
        let (found, neighbors) = {
            let v = View { txn: w, s, metric };
            let found = search_layer(
                &v,
                h.next_node as usize,
                q,
                ep,
                h.ef_construction,
                l,
                &|_| true,
            )?;
            let neighbors = select_neighbors(&v, &found, h.m)?;
            (found, neighbors)
        };
        s.links
            .put(w, &link_key(node, l as u8), &encode_ids(&neighbors))?;
        for &n in &neighbors {
            link(w, s, metric, n, node, l, max)?;
        }
        ep = found[0];
    }
    if level > h.max_level {
        h.max_level = level;
        h.entry = Some(node);
    }
    Ok(())
}

/// Adds `to` to `from`'s links on `level`, pruning with the heuristic when over capacity.
fn link(
    w: &mut RwTxn,
    s: &Store,
    metric: Metric,
    from: u32,
    to: u32,
    level: usize,
    max: usize,
) -> Result<()> {
    let updated = {
        let v = View { txn: w, s, metric };
        let mut current: Vec<u32> = ids(v.links(from, level)?).collect();
        current.push(to);
        if current.len() > max {
            let base = decode_f32s(v.vector(from)?);
            let mut scored = current
                .iter()
                .map(|&n| {
                    Ok(Scored {
                        dist: v.dist(&base, n)?,
                        id: n,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            scored.sort();
            current = select_neighbors(&v, &scored, max)?;
        }
        encode_ids(&current)
    };
    Ok(s.links.put(w, &link_key(from, level as u8), &updated)?)
}
