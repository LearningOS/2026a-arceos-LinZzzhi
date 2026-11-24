//! Collections available to ArceOS applications.
//!
//! This module re-exports all collections from [`alloc::collections`] and adds
//! a `HashMap` implementation, since `alloc` does not provide one.

pub use alloc::collections::*;

use alloc::vec::Vec;
use core::borrow::Borrow;
use core::hash::{BuildHasher, Hash, Hasher};

/// A deterministic FNV-1a hasher.
///
/// It is fast and does not require external crates. It is not hardened against
/// adversarial inputs, which is acceptable for a kernel/user runtime.
#[derive(Clone, Copy)]
pub struct DefaultHasher(u64);

impl DefaultHasher {
    /// Creates a new hasher with the FNV-1a offset basis.
    pub const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl Default for DefaultHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl Hasher for DefaultHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

impl BuildHasher for DefaultHasher {
    type Hasher = Self;

    fn build_hasher(&self) -> Self::Hasher {
        DefaultHasher::new()
    }
}

/// A hash map implemented with separate chaining.
///
/// The interface is a subset of [`std::collections::HashMap`] that covers the
/// operations commonly used by ArceOS applications.
pub struct HashMap<K, V, S = DefaultHasher> {
    buckets: Vec<Vec<(K, V)>>,
    len: usize,
    build_hasher: S,
}

impl<K, V> HashMap<K, V, DefaultHasher> {
    /// Creates an empty `HashMap`.
    pub fn new() -> Self {
        Self::with_hasher(DefaultHasher::new())
    }

    /// Creates an empty `HashMap` with at least the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_capacity_and_hasher(capacity, DefaultHasher::new())
    }
}

impl<K, V, S: Default> Default for HashMap<K, V, S> {
    fn default() -> Self {
        Self::with_hasher(S::default())
    }
}

impl<K, V, S> HashMap<K, V, S> {
    /// Creates an empty `HashMap` using the given hasher.
    pub fn with_hasher(build_hasher: S) -> Self {
        Self {
            buckets: Vec::new(),
            len: 0,
            build_hasher,
        }
    }

    /// Creates an empty `HashMap` with the given capacity and hasher.
    pub fn with_capacity_and_hasher(capacity: usize, build_hasher: S) -> Self {
        // Keep the load factor below 75%.
        let num_buckets = (capacity * 4 / 3).max(1).next_power_of_two();
        Self {
            buckets: (0..num_buckets).map(|_| Vec::new()).collect(),
            len: 0,
            build_hasher,
        }
    }

    /// Returns the number of entries in the map.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Removes all entries from the map.
    pub fn clear(&mut self) {
        for bucket in self.buckets.iter_mut() {
            bucket.clear();
        }
        self.len = 0;
    }

    /// Returns an iterator over the entries of the map.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.buckets.iter().flat_map(|bucket| bucket.iter().map(|(k, v)| (k, v)))
    }

    /// Returns an iterator over the keys of the map.
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.iter().map(|(k, _)| k)
    }

    /// Returns an iterator over the values of the map.
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, v)| v)
    }
}

impl<K: Eq + Hash, V, S: BuildHasher> HashMap<K, V, S> {
    fn bucket_index<Q: Hash + Eq + ?Sized>(&self, key: &Q) -> usize
    where
        K: Borrow<Q>,
    {
        let mut hasher = self.build_hasher.build_hasher();
        key.hash(&mut hasher);
        hasher.finish() as usize % self.buckets.len()
    }

    fn grow(&mut self) {
        let num_buckets = (self.buckets.len() * 2).max(16);
        let mut buckets: Vec<Vec<(K, V)>> = (0..num_buckets).map(|_| Vec::new()).collect();
        for (key, value) in self.buckets.drain(..).flatten() {
            let index = {
                let mut hasher = self.build_hasher.build_hasher();
                key.hash(&mut hasher);
                hasher.finish() as usize % num_buckets
            };
            buckets[index].push((key, value));
        }
        self.buckets = buckets;
    }

    /// Inserts a key-value pair, returning the old value if the key existed.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if self.buckets.is_empty() || self.len + 1 > self.buckets.len() * 3 / 4 {
            self.grow();
        }
        let index = self.bucket_index(&key);
        for (k, v) in self.buckets[index].iter_mut() {
            if *k == key {
                return Some(core::mem::replace(v, value));
            }
        }
        self.buckets[index].push((key, value));
        self.len += 1;
        None
    }

    /// Returns a reference to the value associated with `key`.
    pub fn get<Q: Hash + Eq + ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        if self.buckets.is_empty() {
            return None;
        }
        let index = self.bucket_index(key);
        self.buckets[index]
            .iter()
            .find(|(k, _)| k.borrow() == key)
            .map(|(_, v)| v)
    }

    /// Returns a mutable reference to the value associated with `key`.
    pub fn get_mut<Q: Hash + Eq + ?Sized>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
    {
        if self.buckets.is_empty() {
            return None;
        }
        let index = self.bucket_index(key);
        self.buckets[index]
            .iter_mut()
            .find(|(k, _)| k.borrow() == key)
            .map(|(_, v)| v)
    }

    /// Returns `true` if the map contains `key`.
    pub fn contains_key<Q: Hash + Eq + ?Sized>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
    {
        self.get(key).is_some()
    }

    /// Removes `key` from the map and returns its value.
    pub fn remove<Q: Hash + Eq + ?Sized>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
    {
        if self.buckets.is_empty() {
            return None;
        }
        let index = self.bucket_index(key);
        let bucket = &mut self.buckets[index];
        if let Some(pos) = bucket.iter().position(|(k, _)| k.borrow() == key) {
            self.len -= 1;
            Some(bucket.swap_remove(pos).1)
        } else {
            None
        }
    }
}

impl<K: Eq + Hash, V, S: BuildHasher + Default> FromIterator<(K, V)> for HashMap<K, V, S> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut map = HashMap::with_hasher(S::default());
        for (k, v) in iter {
            map.insert(k, v);
        }
        map
    }
}

impl<K: Eq + Hash, V, S: BuildHasher + Default> Extend<(K, V)> for HashMap<K, V, S> {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}