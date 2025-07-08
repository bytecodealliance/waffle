//! Pooled list data structure.
//!
//! The IR for a function contains many small lists: the lists of
//! arguments and result values for each operator, and the list of
//! result types for each operator as well. It would be fairly
//! inefficient to manage these lists as many separate memory
//! allocations, each with the overhead of a `Vec` (24 bytes on a
//! 64-bit system) in addition to the storage block. So, instead, we
//! aggregate these lists by keeping them all in one large `Vec` (per
//! kind) and holding *index ranges* as virtual handles to lists in
//! the rest of the IR.
//!
//! We define a general abstraction here `ListPool<T>` for a list of
//! `T`, with a `ListRef<T>` that together with the pool can yield an
//! actual slice. This container is instantiated several times in the
//! `FunctionBody`, namely for the `arg_pool` and `type_pool`.

use std::convert::TryFrom;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::{Index, IndexMut};

/// A "storage pool" backing many `ListRef`s of the given type.
#[derive(Clone, Debug)]
pub struct ListPool<T: Clone + Debug> {
    storage: Vec<T>,
}

impl<T: Clone + Debug> Default for ListPool<T> {
    fn default() -> Self {
        ListPool { storage: vec![] }
    }
}

/// A handle to a list stored in a `ListPool`.
///
/// The handle can be used to yield the actual slice, given the pool,
/// but has much smaller overhead than a separately-owned `Vec`: e.g.,
/// 8 bytes on 64-bit systems, rather than 24 bytes, and no separate
/// memory allocation overhead.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ListRef<T>(u32, u32, PhantomData<T>);

impl<T> Default for ListRef<T> {
    fn default() -> Self {
        ListRef(0, 0, PhantomData)
    }
}

impl<T: Clone + Debug> ListPool<T> {
    /// Create a new list in this pool from the items yielded by the
    /// given iterator.
    pub fn from_iter<I: Iterator<Item = T>>(&mut self, iter: I) -> ListRef<T> {
        let start = u32::try_from(self.storage.len()).unwrap();
        self.storage.extend(iter);
        let end = u32::try_from(self.storage.len()).unwrap();
        ListRef(start, end, PhantomData)
    }
    /// Convenience method: create a list from a single item.
    pub fn single(&mut self, value: T) -> ListRef<T> {
        self.from_iter(std::iter::once(value))
    }
    /// Convenience methodS: create a list from exactly two items.
    pub fn double(&mut self, a: T, b: T) -> ListRef<T> {
        self.from_iter(std::iter::once(a).chain(std::iter::once(b)))
    }
    /// Convenience method: create a list from exactly three items.
    pub fn triple(&mut self, a: T, b: T, c: T) -> ListRef<T> {
        self.from_iter(
            std::iter::once(a)
                .chain(std::iter::once(b))
                .chain(std::iter::once(c)),
        )
    }
    /// Allocate a list of the given size with `size` copies of the
    /// value `initial`.
    pub fn allocate(&mut self, size: usize, initial: T) -> ListRef<T> {
        if size == 0 {
            ListRef::default()
        } else {
            self.from_iter(std::iter::repeat(initial).take(size))
        }
    }
    /// Perform a deep-clone of a list: copy it to a new list and
    /// return the handle of that list.
    pub fn deep_clone(&mut self, list: ListRef<T>) -> ListRef<T> {
        self.storage.reserve(list.len());
        let start = u32::try_from(self.storage.len()).unwrap();
        for i in list.0..list.1 {
            self.storage.push(self.storage[i as usize].clone());
        }
        let end = u32::try_from(self.storage.len()).unwrap();
        ListRef(start, end, PhantomData)
    }
}

impl<T: Clone + Debug> Index<ListRef<T>> for ListPool<T> {
    type Output = [T];
    fn index(&self, index: ListRef<T>) -> &[T] {
        &self.storage[index.0 as usize..index.1 as usize]
    }
}

impl<T: Clone + Debug> IndexMut<ListRef<T>> for ListPool<T> {
    fn index_mut(&mut self, index: ListRef<T>) -> &mut [T] {
        &mut self.storage[index.0 as usize..index.1 as usize]
    }
}

impl<T> ListRef<T> {
    /// Return the number of items in this list. (We do not need the
    /// pool to compute this.)
    pub fn len(&self) -> usize {
        (self.1 - self.0) as usize
    }
    /// Return whether this list is empty. (We do not need the pool to
    /// compute this.)
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
