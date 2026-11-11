//! Dense growable bitset over internal node ids (ADR004).

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bitset {
    pub(crate) words: Vec<u64>,
}

impl Bitset {
    pub fn with_len(bits: usize) -> Self {
        Self {
            words: vec![0; bits.div_ceil(64)],
        }
    }

    pub fn set(&mut self, i: u32) {
        let w = (i / 64) as usize;
        if w >= self.words.len() {
            self.words.resize(w + 1, 0);
        }
        self.words[w] |= 1 << (i % 64);
    }

    pub fn clear(&mut self, i: u32) {
        if let Some(w) = self.words.get_mut((i / 64) as usize) {
            *w &= !(1 << (i % 64));
        }
    }

    #[inline]
    pub fn contains(&self, i: u32) -> bool {
        self.words
            .get((i / 64) as usize)
            .is_some_and(|w| w & (1 << (i % 64)) != 0)
    }

    pub fn count(&self) -> usize {
        self.words.iter().map(|w| w.count_ones() as usize).sum()
    }

    pub fn or_with(&mut self, other: &Bitset) {
        if other.words.len() > self.words.len() {
            self.words.resize(other.words.len(), 0);
        }
        self.words
            .iter_mut()
            .zip(&other.words)
            .for_each(|(a, b)| *a |= b);
    }

    pub fn and_with(&mut self, other: &Bitset) {
        self.words.truncate(other.words.len());
        self.words
            .iter_mut()
            .zip(&other.words)
            .for_each(|(a, b)| *a &= b);
    }

    pub fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        self.words.iter().enumerate().flat_map(|(wi, &w)| {
            let mut bits = w;
            std::iter::from_fn(move || {
                if bits == 0 {
                    return None;
                }
                let b = bits.trailing_zeros();
                bits &= bits - 1;
                Some(wi as u32 * 64 + b)
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_ops_and_iteration() {
        let mut a = Bitset::default();
        [1, 64, 65, 200].iter().for_each(|&i| a.set(i));
        let mut b = Bitset::default();
        [64, 200, 300].iter().for_each(|&i| b.set(i));
        let mut and = a.clone();
        and.and_with(&b);
        assert_eq!(and.iter().collect::<Vec<_>>(), vec![64, 200]);
        let mut or = a.clone();
        or.or_with(&b);
        assert_eq!(or.count(), 5);
        a.clear(64);
        assert!(!a.contains(64) && a.contains(65) && !a.contains(10_000));
    }
}
