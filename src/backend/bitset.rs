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
