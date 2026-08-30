---
issue: 209
title: "Bin streaming: owned decode, the single decode path, and the lookup cache"
labels: crate:ltk_meta, enhancement, format:bin, area:reading, blocked
---

Part of #192 (design: `docs/design/bin-streaming.md` section 4.2, section 4.4, section 9). The owned escape hatches, the parser unification, and the opt-in lookup cache.

## Proposed surface

```rust
impl<'a, R: io::Read + io::Seek, M: Default> ObjectStream<'a, R, M> {
    /// Parses the whole object into an eager [`BinObject`]. (`read`, not `parse`:
    /// it does I/O, and the crate's vocabulary is `from_reader` / `ReadProperty`.)
    /// Equivalent to `view()` plus an owned decode through the wire core.
    pub fn read(&mut self) -> Result<BinObject<M>, Error>;
}

impl<R: io::Read + io::Seek, M: Default> BinStream<R, M> {
    /// Parses the remaining file into an eager [`Bin`], consuming the handle.
    ///
    /// Always processes the whole object table from the top. Size mismatches are
    /// [`Error::InvalidSize`], exactly as `Bin::from_reader` errors today.
    pub fn into_bin(self) -> Result<Bin<M>, Error>;

    /// Resolves an object through the installed [`ObjectCache`]: a hit is an `Arc`
    /// clone with no I/O, a miss parses and inserts. Under the default [`NoCache`]
    /// this parses on every call.
    pub fn cached_object(&mut self, path_hash: impl Into<BinHash>)
        -> Result<Option<Arc<BinObject<M>>>, Error>;

    /// Installs a cache provider. The default is [`NoCache`].
    pub fn set_cache(&mut self, cache: Box<dyn ObjectCache<M> + Send>);
}
```

`Bin::from_reader` becomes mount + drain, making the stream the crate's only parser:

```rust
pub fn from_reader<R: io::Read + io::Seek + ?Sized>(reader: &mut R) -> Result<Self, Error> {
    BinStream::mount(&mut *reader)?.into_bin()
}
```

The existing `ReadProperty` impls are rebuilt over the wire core's `&[u8]` codecs (contract phase of the refactor started in #207); the top-level loops exist once, in the stream. Their inline size checks (`Container::from_reader` measuring its own body) move into the walk layer, which raises the same `Error::InvalidSize` — one check, one place, and the eager API's behavior is unchanged. The homogeneity checks (`InvalidNesting`, `InvalidKeyType`, `MismatchedContainerTypes`) stay in the value model: the stream parses through the same checked constructors, so there is never a second, unchecked way to build a container.

## The cache

The provider *is* the policy — no policy enum, no third type parameter on the handle:

```rust
/// A lookup cache for parsed objects. The provider owns its eviction policy.
pub trait ObjectCache<M> {
    fn get(&mut self, key: BinHash) -> Option<Arc<BinObject<M>>>;
    fn put(&mut self, key: BinHash, value: Arc<BinObject<M>>);
    fn clear(&mut self);
}

/// The default provider: caches nothing. `get` is always a miss, `put` drops.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoCache;

/// Least-recently-used cache bounded by object count.
#[derive(Debug)]
pub struct LruObjectCache { /* capacity: NonZeroUsize, … */ }

impl LruObjectCache {
    pub fn new(capacity: NonZeroUsize) -> Self;
}
```

- `Arc<BinObject<M>>` is the currency: a hit is an `Arc` clone, eviction never invalidates anything, values cross threads.
- The box requires `Send`, so a handle with a cache installed stays `Send`. `Rc`-based providers are ruled out, deliberately.
- Only `cached_object()` consults it — sweeps and `object()` never touch the cache, so a sweep does not evict what a consumer is holding hot.

Demoable: the full existing test suite passes with `from_reader` running through the stream, plus an install-wide equivalence sweep.

Blocked by #207. Can run in parallel with #208.

- [ ] All existing `ltk_meta` snapshot and round-trip tests pass unmodified
- [ ] Corpus sweep: `into_bin()` equals the pre-change eager parse for every PROP chunk in an install
- [ ] A file the old reader rejected with the size error is still rejected by `from_reader`, with the same variant
- [ ] Homogeneity errors (nesting, key type, mismatched container types) still raise from the same inputs
- [ ] `cached_object` hit returns without I/O; eviction never invalidates a held `Arc`; handle stays `Send` with a cache installed
- [ ] Under `NoCache`, `cached_object` parses per call (documented, tested)
