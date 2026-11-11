//! Bounded top-k selection ordered by (distance, internal id) for deterministic ties.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scored {
    pub dist: f32,
    pub id: u32,
}

impl Eq for Scored {}
impl Ord for Scored {
    fn cmp(&self, o: &Self) -> Ordering {
        self.dist.total_cmp(&o.dist).then(self.id.cmp(&o.id))
    }
}
impl PartialOrd for Scored {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// Keeps the k smallest items (max-heap of the current best).
pub struct TopK {
    k: usize,
    heap: BinaryHeap<Scored>,
}

impl TopK {
    pub fn new(k: usize) -> Self {
        Self {
            k,
            heap: BinaryHeap::with_capacity(k + 1),
        }
    }
    #[inline]
    pub fn push(&mut self, s: Scored) {
        if self.heap.len() < self.k {
            self.heap.push(s);
        } else if self.k > 0 && s < *self.heap.peek().unwrap() {
            self.heap.pop();
            self.heap.push(s);
        }
    }
    pub fn into_sorted(self) -> Vec<Scored> {
        self.heap.into_sorted_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_smallest_k_sorted_with_id_tiebreak() {
        let mut t = TopK::new(3);
        for (d, id) in [(0.5, 1), (0.1, 2), (0.9, 3), (0.1, 0), (0.3, 4)] {
            t.push(Scored { dist: d, id });
        }
        let ids: Vec<u32> = t.into_sorted().iter().map(|s| s.id).collect();
        assert_eq!(ids, vec![0, 2, 4]);
    }
}
