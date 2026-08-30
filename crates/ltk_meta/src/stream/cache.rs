//! The opt-in lookup cache a [`BinStream`](crate::stream::BinStream) resolves through.
//!
//! Repeated lookups into one file — a bin editor chasing `ObjectLink`s, a manager resolving the
//! same scene objects across requests — should not re-parse. The handle holds one provider, and
//! the provider *is* the policy: there is no policy enum and no third type parameter.
//!
//! Only [`BinStream::cached_object`](crate::stream::BinStream::cached_object) consults it. The
//! cursors and [`BinStream::object`](crate::stream::BinStream::object) never do, so a sweep
//! cannot evict what a consumer is holding hot.

use std::{num::NonZeroUsize, sync::Arc};

use indexmap::IndexMap;
use ltk_hash::BinHash;

use crate::{property::NoMeta, BinObject};

/// A lookup cache for parsed objects. The provider owns its eviction policy.
///
/// [`Arc<BinObject<M>>`](Arc) is the currency: a hit is an `Arc` clone, so callers keep values
/// as long as they like, eviction never invalidates anything, and the values cross threads.
pub trait ObjectCache<M> {
    /// The cached object for `key`, if the provider holds one.
    fn get(&mut self, key: BinHash) -> Option<Arc<BinObject<M>>>;

    /// Offers `value` to the provider, which may keep it or drop it.
    fn put(&mut self, key: BinHash, value: Arc<BinObject<M>>);

    /// Drops everything the provider holds.
    fn clear(&mut self);
}

/// The default provider: caches nothing.
///
/// [`ObjectCache::get`] is always a miss and [`ObjectCache::put`] drops, so
/// [`BinStream::cached_object`](crate::stream::BinStream::cached_object) parses on every call.
/// A real provider rather than an `Option`, so the handle has one mechanism and no
/// special-cased disabled state.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoCache;

impl<M> ObjectCache<M> for NoCache {
    fn get(&mut self, _key: BinHash) -> Option<Arc<BinObject<M>>> {
        None
    }

    fn put(&mut self, _key: BinHash, _value: Arc<BinObject<M>>) {}

    fn clear(&mut self) {}
}

/// Least-recently-used cache, bounded by object count.
///
/// Recency is the map's own order, so a hit and an eviction each shift the entries after the
/// one they touch. For the handful-to-hundreds capacities this is meant for, that is noise
/// beside the parse it saves.
///
/// # Examples
///
/// ```no_run
/// use std::{fs::File, num::NonZeroUsize};
/// use ltk_meta::{concrete::BinStream, stream::LruObjectCache};
///
/// let mut stream = BinStream::mount(File::open("data.bin")?)?;
/// stream.set_cache(Box::new(LruObjectCache::new(NonZeroUsize::new(64).unwrap())));
///
/// // The second lookup of the same object costs no I/O and no parse.
/// let first = stream.cached_object(0x1234_5678_u32)?;
/// let again = stream.cached_object(0x1234_5678_u32)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct LruObjectCache<M = NoMeta> {
    capacity: NonZeroUsize,
    /// Least recently used first.
    entries: IndexMap<BinHash, Arc<BinObject<M>>>,
}

impl<M> LruObjectCache<M> {
    /// An empty cache that holds at most `capacity` objects.
    #[must_use]
    pub fn new(capacity: NonZeroUsize) -> Self {
        Self {
            capacity,
            entries: IndexMap::new(),
        }
    }

    /// How many objects the cache holds at most.
    #[must_use]
    pub fn capacity(&self) -> NonZeroUsize {
        self.capacity
    }

    /// How many objects the cache is holding.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is holding nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<M> ObjectCache<M> for LruObjectCache<M> {
    fn get(&mut self, key: BinHash) -> Option<Arc<BinObject<M>>> {
        let index = self.entries.get_index_of(&key)?;
        let (_, value) = self.entries.shift_remove_index(index)?;
        self.entries.insert(key, Arc::clone(&value));
        Some(value)
    }

    fn put(&mut self, key: BinHash, value: Arc<BinObject<M>>) {
        self.entries.shift_remove(&key);
        if self.entries.len() >= self.capacity.get() {
            self.entries.shift_remove_index(0);
        }
        self.entries.insert(key, value);
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::concrete;

    fn object(path_hash: u32) -> Arc<concrete::BinObject> {
        Arc::new(concrete::BinObject::new(path_hash, 0xAAAAu32))
    }

    fn capacity(n: usize) -> NonZeroUsize {
        NonZeroUsize::new(n).expect("a non-zero capacity")
    }

    #[test]
    fn no_cache_never_holds_anything() {
        let mut cache = NoCache;
        cache.put(0x1u32.into(), object(0x1));
        assert!(ObjectCache::<NoMeta>::get(&mut cache, 0x1u32.into()).is_none());
    }

    #[test]
    fn evicts_the_least_recently_used() {
        let mut cache = LruObjectCache::new(capacity(2));
        cache.put(0x1u32.into(), object(0x1));
        cache.put(0x2u32.into(), object(0x2));

        // Touching 1 makes 2 the oldest, so inserting 3 drops 2.
        assert!(cache.get(0x1u32.into()).is_some());
        cache.put(0x3u32.into(), object(0x3));

        assert_eq!(cache.len(), 2);
        assert!(cache.get(0x1u32.into()).is_some());
        assert!(cache.get(0x2u32.into()).is_none());
        assert!(cache.get(0x3u32.into()).is_some());
    }

    /// Eviction hands out no invalidation: a held `Arc` outlives the entry it came from.
    #[test]
    fn an_evicted_value_stays_alive_for_its_holder() {
        let mut cache = LruObjectCache::new(capacity(1));
        cache.put(0x1u32.into(), object(0x1));

        let held = cache.get(0x1u32.into()).expect("a hit");
        cache.put(0x2u32.into(), object(0x2));

        assert!(cache.get(0x1u32.into()).is_none(), "1 was evicted");
        assert_eq!(held.path_hash, 0x1u32.into());
    }

    #[test]
    fn re_inserting_a_key_does_not_grow_the_cache() {
        let mut cache = LruObjectCache::new(capacity(2));
        cache.put(0x1u32.into(), object(0x1));
        cache.put(0x1u32.into(), object(0x1));

        assert_eq!(cache.len(), 1);

        cache.clear();
        assert!(cache.is_empty());
    }
}
