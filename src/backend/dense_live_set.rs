//! A dense, epoch-based live-set used during block walks.
//!
//! # Epoch-based O(1) clearing
//!
//! The `DenseLiveSet` is reused across multiple block visits without
//! reallocating or clearing its backing storage.
//!
//! We want a dense set because accesses are much faster than a HashSet.
//! We would not want to allocate a dense set separately for every block
//! we process, however (as we get further down in the function, there
//! would be a larger and larger "prefix" of unused IDs). Instead we
//! want to use one allocation for a whole scan through a function body.
//!
//! But we cannot clear a dense set in O(1) if its per-index entries are
//! just booleans; that entails scanning through the whole thing with a
//! bulk memset (or at least, a walk through the dirty-index list and a
//! store at each index). So instead, this is achieved through an epoch
//! counter:
//!
//! * Each slot in the `mark` array stores `epoch * 2 + state`, where
//!   `state` is `0` (live) or `1` (dead).
//! * When a new block walk begins, `next_epoch()` increments the epoch
//!   counter and clears the `touched` list — both O(1) operations.
//! * Because the old epoch values in `mark` no longer match the current
//!   epoch, stale entries from previous block visits are automatically
//!   treated as absent without needing to zero out the array.

/// A dense live-set with O(1) clearing via epoch tagging.
#[derive(Debug)]
pub struct DenseLiveSet {
    /// Per-slot mark: `epoch * 2 + state` where state is 0 (live) or 1
    /// (dead).  Slots with a mark from a previous epoch are treated as
    /// absent.
    mark: Vec<u64>,
    /// The current epoch counter. Incremented on `next_epoch()` to
    /// logically clear the set.
    epoch: u64,
    /// Indices touched (set_live or set_dead) during the current epoch.
    /// Cleared on each `next_epoch()` call.
    touched: Vec<u32>,
}

impl DenseLiveSet {
    /// Create a new set with space for `n` indices.
    pub fn new(n: usize) -> Self {
        Self {
            mark: vec![0; n],
            epoch: 0,
            touched: Vec::new(),
        }
    }

    /// Start a new epoch, logically clearing the set in O(1).
    ///
    /// Increments the epoch counter and clears the touched-index list.
    pub fn next_epoch(&mut self) {
        self.epoch += 1;
        self.touched.clear();
    }

    /// Mark index `i` as live.
    ///
    /// Returns `true` if it was already live in this epoch (i.e., this
    /// is not a first-use).
    pub fn set_live(&mut self, i: u32) -> bool {
        let m = &mut self.mark[i as usize];
        let newly_touched = *m / 2 != self.epoch;
        if newly_touched {
            self.touched.push(i);
        }
        let was_live = !newly_touched && *m % 2 == 0;
        *m = self.epoch * 2;
        !was_live
    }

    /// Mark index `i` as dead (retired by a def).
    ///
    /// Returns `true` if it was live in this epoch.
    pub fn set_dead(&mut self, i: u32) -> bool {
        let m = &mut self.mark[i as usize];
        if *m / 2 != self.epoch {
            self.touched.push(i);
            *m = self.epoch * 2 + 1;
            return false;
        }
        let was_live = *m % 2 == 0;
        *m = self.epoch * 2 + 1;
        was_live
    }

    /// Check if index `i` is currently live in this epoch.
    pub fn is_live(&self, i: u32) -> bool {
        let m = self.mark[i as usize];
        m / 2 == self.epoch && m % 2 == 0
    }

    /// Returns the list of indices touched during the current epoch.
    ///
    /// Useful for iterating only over entries that were modified in the
    /// current epoch, avoiding a full scan of all possible indices.
    pub fn touched(&self) -> &[u32] {
        &self.touched
    }
}
