use std::convert::TryFrom;

/// A lazily-grown bitset.
#[derive(Clone, Debug, Default)]
pub struct Bitset {
    words: Vec<u64>,
}

impl Bitset {
    /// Set bit `i`; returns whether it was newly set.
    pub fn insert(&mut self, i: u32) -> bool {
        let word = usize::try_from(i / 64).unwrap();
        let bit = 1u64 << (i % 64);
        if word >= self.words.len() {
            self.words.resize(word + 1, 0);
        }
        let w = &mut self.words[word];
        let new = *w & bit == 0;
        *w |= bit;
        new
    }

    /// Iterate over all set bit-indices.
    pub fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        self.words.iter().enumerate().flat_map(|(wi, &w)| {
            let mut w = w;
            std::iter::from_fn(move || {
                if w == 0 {
                    return None;
                }
                let bit = w.trailing_zeros();
                w &= w - 1;
                let limb_start = u32::try_from(wi).unwrap() * 64;
                Some(limb_start + bit)
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        let b = Bitset::default();
        assert_eq!(b.iter().collect::<Vec<_>>(), Vec::<u32>::new());
    }

    #[test]
    fn insert_returns_whether_newly_set() {
        let mut b = Bitset::default();
        assert!(b.insert(0));
        assert!(!b.insert(0));
        assert!(b.insert(63));
        assert!(b.insert(64));
        assert!(!b.insert(63));
        assert!(!b.insert(64));
    }

    #[test]
    fn iter_single_word() {
        let mut b = Bitset::default();
        for i in [3, 0, 17, 63] {
            b.insert(i);
        }
        assert_eq!(b.iter().collect::<Vec<_>>(), vec![0, 3, 17, 63]);
    }

    #[test]
    fn iter_across_words() {
        let mut b = Bitset::default();
        for i in [200, 64, 63, 1, 128] {
            b.insert(i);
        }
        assert_eq!(b.iter().collect::<Vec<_>>(), vec![1, 63, 64, 128, 200]);
    }

    #[test]
    fn word_boundaries() {
        let mut b = Bitset::default();
        for i in [0, 63, 64, 127, 128, 191, 192] {
            assert!(b.insert(i));
        }
        assert_eq!(
            b.iter().collect::<Vec<_>>(),
            vec![0, 63, 64, 127, 128, 191, 192]
        );
    }

    #[test]
    fn sparse_growth() {
        let mut b = Bitset::default();
        assert!(b.insert(10_000));
        assert!(b.insert(5));
        assert_eq!(b.iter().collect::<Vec<_>>(), vec![5, 10_000]);
    }

    #[test]
    fn duplicate_inserts_do_not_duplicate_in_iter() {
        let mut b = Bitset::default();
        b.insert(42);
        b.insert(42);
        b.insert(42);
        assert_eq!(b.iter().collect::<Vec<_>>(), vec![42]);
    }
}
