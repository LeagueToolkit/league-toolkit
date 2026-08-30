# Streaming bin reading in `ltk_meta`

Design for review. Covers [#192](https://github.com/LeagueToolkit/league-toolkit/issues/192)
(lazy bin reading).

Status: design session held 2026-08-30; the critical decisions below are agreed, the surface
is drafted for review. Nothing is implemented. Sequenced behind
[#187](https://github.com/LeagueToolkit/league-toolkit/pull/187), which rewrites the value model
this reads into - see section 12.

## 1. Summary

`ltk_meta` today reads a `.bin` all-or-nothing: `Bin::from_reader` parses every property of
every object into a tree. That is the wrong shape for the consumers #192 names — harvesting
object path hashes for a grep index over 42,306 files, reading a header without touching the
body, resolving one object out of a file on demand.

The client's own loader is the model. `MetaFile_readEntry` is a one-pass streaming reader: it
walks the object table front to back, deserializes each sized entry as it arrives, and uses
the size fields only to seek past what it will not parse (dead-listed entries, unresolvable
classes). It never builds a whole-file tree. Streaming is therefore the canonical reading
model for this format, and the eager `Bin` is the derived convenience — the new API is named
and layered accordingly.

The proposal adds a `ltk_meta::stream` module:

- **`BinStream<R: Read + Seek>`** — an owning handle over a `PROP` stream, mounted the way
  `ltk_wad::Wad` is. Mounting reads the header, dependencies and class-hash table (all
  sequential, no seeking) and stops.
- **A cursor at the file level, zero-copy views at the object level.** `objects()` sweeps
  the object table yielding one `ObjectStream` at a time; what is not descended into is
  skipped by size, exactly as the client skips. Descending buffers the object's declared
  byte range (one read, reused buffer) and hands back an `ObjectView` — a lazily-decoded
  view over those bytes: `std` iterators over properties, a borrowed `ValueView` mirror of
  `PropertyValueEnum` to any depth, nothing materialized until asked for. Owning is one
  call away (`read()` → `BinObject`, `value()` → `PropertyValueEnum`).
- **A plain-data layer over the cursors.** `entries()` is a `std::iter::Iterator` of
  `ObjectEntry` descriptors for ergonomic harvesting; `object(path_hash)` gives random
  access, building the offset table transparently on first use.
- **Opt-in lookup caching.** The handle holds a cache provider (`NoCache` by default) and
  `cached_object()` hands out `Arc<BinObject<M>>`, so a consumer resolving the same objects
  repeatedly pays each parse once.
- **`BinOverrideStream<R>`** — the same treatment for `PTCH` files, including streaming the
  patch records that the eager reader also parses, and exposing the outer header's delete
  list.
- **One decode path.** `Bin::from_reader` is reimplemented as mount + drain
  (`into_bin`), so the stream is the only parser and the two can never drift.

## 2. Decisions from the design session

| Decision             | Choice                                                                                                                                                                                                                      |
| -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Handle model         | Owns the reader: `BinStream<R: Read + Seek>::mount(source)`                                                                                                                                                                 |
| Scan timing          | Lazy and transparent: mount reads header + class table only; the offset index is built on first random access; iteration streams without it                                                                                 |
| Laziness granularity | Value-level, to any depth: an object's bytes are buffered once and viewed zero-copy; `ValueView` descends without materializing (revised — section 14)                                                                      |
| Property access      | `std` iterators and random access over the buffered `ObjectView`; owned values on request (revised from forward-only cursors — section 14)                                                                                  |
| Iteration shape      | Streaming cursor at the file level; plain-descriptor `std` iterator over it; buffered views inside an object (revised — section 14)                                                                                         |
| Wire core            | One byte-level module owns offsets, shapes, skips and leaf codecs; the eager `ReadProperty` impls and the views are two renderers over it (section 9)                                                                       |
| Naming               | `BinStream` / `BinOverrideStream` / `BinFileStream` in `ltk_meta::stream`, `mount()` constructor matching `Wad`                                                                                                             |
| Layering             | Single decode path: `Bin::from_reader` becomes `BinStream::mount` + `into_bin`                                                                                                                                              |
| Strictness           | Counts drive the parse, declared sizes drive the skips; a size that disagrees with the count-driven walk is `Error::InvalidSize`, the same error the eager readers raise (revised — section 7)                              |
| Scope                | `PROP` and `PTCH`, reading only; `into_bin()` upgrade; the rebase/delta (surgical rewrite) pipeline is a later stage this design must not preclude                                                                          |
| Metadata parameter   | Handle-level: `BinStream<R, M = NoMeta>`, pinned once through `concrete` aliases — revisited from the first draft's method-level `M`, see section 12                                                                        |
| Buffering            | The handle wraps its source in `BufReader` and seeks with `seek_relative`; callers hand over the bare `File`                                                                                                                |
| Caching              | Opt-in provider on the handle: `Box<dyn ObjectCache<M> + Send>`, `NoCache` default, `LruObjectCache` shipped, `Arc<BinObject<M>>` returns — section 4.4                                                                     |
| Error surface        | `Error` gains `#[non_exhaustive]` in the 0.8.0 release #187 opens, so the stream (and later work) grows variants in minor releases                                                                                          |
| Write-back           | An edit saves as a rewritten `.bin` via the delta contract (section 15): untouched objects raw-copied byte-exactly, edited ones re-encoded. PTCH authoring is explicitly out of scope until that layer is designed for mods |

## 3. Wire facts the design leans on

- **The class table is free, path hashes are not.** The object table is
  `u32 count`, then `count × u32 class_hash`, then `count × (u32 size, u32 path_hash,
  u16 prop_count, properties…)`. After the sequential header read the handle already holds
  every class hash. Harvesting path hashes takes one seek-hop per object reading 8 bytes
  (`size`, `path_hash`), which is also the moment each object's `(offset, size)` is learned —
  so the sweep that harvests is the sweep that indexes.
- **Every complex value carries its byte size.** Objects, `Struct`/`Embedded`, containers and
  maps store a size ahead of their body; primitives have fixed widths and strings a length
  prefix. Skipping any unparsed value is therefore a seek, mirroring `MetaValue_skipByType`.
- **The client never verifies sizes on the parse path.** It trusts counts when parsing and
  reads sizes only to skip. `ltk_meta`'s eager reader measures every region and errors on
  mismatch. The stream takes the client's semantics (section 7).
- **Legacy property-kind numbering is detectable only by parsing.** The eager reader retries
  the whole object table in legacy numbering when a kind byte fails to decode. A streaming
  reader discovers this mid-sweep; section 8 defines the latch.

## 4. API surface — `PROP`

All types live in `ltk_meta::stream` and are re-exported from the crate root. Signatures are
the design; doc comments are abbreviated. `M` is the same property-meta parameter the eager
types carry, defaulting to `NoMeta`, and lives on the handle: `concrete` grows `BinStream`,
`BinOverrideStream` and `BinFileStream` aliases that pin it at the `mount` call, after which
it disappears from every downstream signature (section 12 has the reasoning).

```rust
/// A mounted `PROP` stream: the header is parsed, the object table is read on demand.
///
/// Owns its source and buffers internally (`BufReader` + `seek_relative`, so the sweep's
/// short hops stay inside the buffer). Hand it the bare `File`; pre-wrapping in
/// `BufReader` only double-buffers.
pub struct BinStream<R: io::Read + io::Seek, M = NoMeta> {
    /* buffered reader, header, lazy toc, latch, cache */
}

impl<R: io::Read + io::Seek, M: Default> BinStream<R, M> {
    /// Mounts a `PROP` stream, reading the header, dependencies and class-hash table.
    ///
    /// Reads sequentially to the start of the object bodies and stops. Returns
    /// [`Error::UnexpectedBinKind`] for a `PTCH` stream.
    pub fn mount(source: R) -> Result<Self, Error>;

    // ── header facts, free after mount ──────────────────────────────────────
    pub fn version(&self) -> u32;
    pub fn dependencies(&self) -> &[String];
    /// Class hash of every object, in file order. `class_hashes().len()` is the object count.
    pub fn class_hashes(&self) -> &[BinHash];

    // ── sweeping ────────────────────────────────────────────────────────────
    /// A cursor over the object table. Every call starts a fresh sweep from the top;
    /// cursors hold no state between calls. Objects not descended into are skipped
    /// by their size field.
    pub fn objects(&mut self) -> Objects<'_, R, M>;

    /// A `std` iterator of plain descriptors, for harvesting and filtering.
    /// Equivalent to `objects()` without ever descending; restarts the same way.
    pub fn entries(&mut self) -> Entries<'_, R, M>;

    // ── random access ───────────────────────────────────────────────────────
    /// The table of contents: every object's `(path_hash, class_hash, offset, size)`.
    ///
    /// Built by one harvest sweep on first use, then cached. `objects()` / `entries()`
    /// sweeps also populate it as a side effect, so a fully-swept handle pays nothing.
    pub fn toc(&mut self) -> Result<&BinToc, Error>;

    /// Opens the object with the given path hash, building the TOC if needed.
    pub fn object(&mut self, path_hash: impl Into<BinHash>)
        -> Result<Option<ObjectStream<'_, R, M>>, Error>;

    // ── cached lookup (section 4.4) ─────────────────────────────────────────
    /// Resolves an object through the installed [`ObjectCache`]: a hit is an `Arc`
    /// clone with no I/O, a miss parses and inserts. Under the default [`NoCache`]
    /// this parses on every call.
    pub fn cached_object(&mut self, path_hash: impl Into<BinHash>)
        -> Result<Option<Arc<BinObject<M>>>, Error>;

    /// Installs a cache provider. The default is [`NoCache`].
    pub fn set_cache(&mut self, cache: Box<dyn ObjectCache<M> + Send>);

    // ── upgrade / teardown ──────────────────────────────────────────────────
    /// Parses the remaining file into an eager [`Bin`], consuming the handle.
    ///
    /// Always processes the whole object table from the top, regardless of cursor
    /// position. Size mismatches are [`Error::InvalidSize`], exactly as
    /// `Bin::from_reader` errors today (section 7).
    pub fn into_bin(self) -> Result<Bin<M>, Error>;

    /// Returns the underlying source, discarding the internal buffer.
    pub fn into_inner(self) -> R;
}
```

```rust
/// One row of the table of contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectEntry {
    pub path_hash: BinHash,
    pub class_hash: BinHash,
    /// Absolute offset of the object's `u32 size` field.
    pub offset: u64,
    /// Declared byte size of the object body (as the file states it).
    pub size: u32,
}

/// File-order entries plus a hash index. Plain data: `Clone`, so a consumer (the
/// ltk-manager object index) can detach it from the handle, and serializable behind
/// the `serde` feature so it can be persisted.
#[derive(Debug, Clone)]
pub struct BinToc { /* Vec<ObjectEntry> + HashMap<BinHash, usize> */ }

impl BinToc {
    pub fn entries(&self) -> &[ObjectEntry];
    pub fn entry(&self, path_hash: impl Into<BinHash>) -> Option<&ObjectEntry>;
}
```

### 4.1 The object cursor

```rust
/// Streaming cursor over the object table. Not a `std` iterator: each yielded
/// [`ObjectStream`] borrows the reader, so the borrow checker enforces one open
/// object at a time.
#[must_use = "cursors are lazy and read nothing until advanced"]
pub struct Objects<'a, R: io::Read + io::Seek, M = NoMeta> { /* &'a mut BinStream<R, M>, position */ }

impl<'a, R: io::Read + io::Seek, M: Default> Objects<'a, R, M> {
    /// Advances to the next object, skipping whatever the previous one did not consume.
    ///
    /// Reads the 8-byte object header (`size`, `path_hash`); the class hash comes from
    /// the table read at mount. Feeds the TOC as it goes.
    pub fn next(&mut self) -> Result<Option<ObjectStream<'_, R, M>>, Error>;
}

/// `std` iterator of plain [`ObjectEntry`] descriptors — `Objects` without descent.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct Entries<'a, R: io::Read + io::Seek, M = NoMeta> { /* Objects<'a, R, M> */ }

impl<R: io::Read + io::Seek, M: Default> Iterator for Entries<'_, R, M> {
    type Item = Result<ObjectEntry, Error>;
    fn next(&mut self) -> Option<Self::Item>;
}
```

### 4.2 One object

```rust
/// A view of one object positioned in the stream. Dropping it without descending
/// costs nothing; the parent cursor skips by size.
pub struct ObjectStream<'a, R: io::Read + io::Seek, M = NoMeta> { /* … */ }

impl<'a, R: io::Read + io::Seek, M: Default> ObjectStream<'a, R, M> {
    pub fn path_hash(&self) -> BinHash;
    pub fn class_hash(&self) -> BinHash;
    pub fn entry(&self) -> ObjectEntry;
    /// The object's raw byte range in the stream (`size` field included), as the
    /// rebase/delta pipeline will need for byte-exact copy-through.
    pub fn byte_range(&self) -> Range<u64>;

    /// Number of properties, read from the object header on first use.
    pub fn property_count(&mut self) -> Result<u16, Error>;

    /// Buffers the object's declared byte range (one read into the handle's reused
    /// buffer) and returns a zero-copy view over it. Everything inside the object —
    /// iteration, random access, descent — happens in memory from here (section 4.3).
    pub fn view(&mut self) -> Result<ObjectView<'_>, Error>;

    /// Parses the whole object into an eager [`BinObject`]. (`read`, not `parse`:
    /// it does I/O, and the crate's vocabulary is `from_reader` / `ReadProperty`.)
    /// Equivalent to `view()` plus an owned decode through the wire core.
    pub fn read(&mut self) -> Result<BinObject<M>, Error>;
}
```

### 4.3 Views

The per-object layer is zero-copy views over the buffered bytes, not cursors over the
reader. Views are plain shared references: `std` iterators, any number of properties held
and compared at once, backtracking free, skipping is slice arithmetic. Nothing decodes
until touched, and nothing allocates until an *owned* value is asked for. The views carry
the handle's `M` as a phantom parameter so the owned-decode escape hatches infer without
turbofish; the borrowed data itself is metadata-free.

```rust
/// One object's bytes, viewed in place.
pub struct ObjectView<'a, M = NoMeta> { /* path/class hashes, &'a [u8], legacy flag */ }

impl<'a, M: Default> ObjectView<'a, M> {
    pub fn path_hash(&self) -> BinHash;
    pub fn class_hash(&self) -> BinHash;
    pub fn property_count(&self) -> u16;

    /// The properties in file order. A real `std` iterator; items are `Result` because
    /// a header's kind byte can fail to decode.
    pub fn properties(&self) -> impl Iterator<Item = Result<PropertyView<'a, M>, Error>>;

    /// Random access by name hash — an in-memory walk, no index needed.
    pub fn property(&self, name_hash: impl Into<BinHash>)
        -> Result<Option<PropertyView<'a, M>>, Error>;

    /// The object's raw bytes, for the rebase pipeline's copy-through.
    pub fn raw(&self) -> &'a [u8];
}

/// One property: header decoded, value untouched.
pub struct PropertyView<'a, M = NoMeta> { /* name hash, kind, value bytes */ }

impl<'a, M: Default> PropertyView<'a, M> {
    pub fn name_hash(&self) -> BinHash;
    pub fn kind(&self) -> PropertyKind;

    /// The value's wire shape — a container's item kind, a map's key and value kinds,
    /// an embed's class — from the few header bytes ahead of the body. Returns the
    /// same [`ValueShape`] the resolver's type rule uses (#187), filled by the rules
    /// of `ValueShape::of` (a pointer's class is not recorded).
    pub fn shape(&self) -> Result<ValueShape, Error>;

    /// For containers and maps, the element count from the same header bytes.
    pub fn item_count(&self) -> Result<Option<u32>, Error>;

    /// The value's raw bytes (header excluded).
    pub fn raw(&self) -> &'a [u8];

    /// Descends into the value without materializing it.
    pub fn value_view(&self) -> Result<ValueView<'a, M>, Error>;

    /// Decodes the value — the whole subtree — into the existing owned representation.
    pub fn value(&self) -> Result<PropertyValueEnum<M>, Error>;
}
```

`ValueView` is the borrowed mirror of `PropertyValueEnum`, one variant per `Kind`, the
same shape `ValueMut` (#187) takes for mutation:

```rust
/// A borrowed, lazily-decoded value. Leaves carry decoded primitives (`&'a str` for
/// strings, validated on access); complex kinds carry sub-views that descend further,
/// still zero-copy, to any depth.
pub enum ValueView<'a, M = NoMeta> {
    None,
    Bool(bool),
    I8(i8), /* … */ U64(u64), F32(f32),
    Vector2(Vec2), Vector3(Vec3), Vector4(Vec4), Matrix44(Mat4),
    Color(Color),
    String(&'a str),
    Hash(BinHash),
    WadChunkLink(WadHash),
    ObjectLink(BinHash),
    BitBool(bool),
    Container(ContainerView<'a, M>),
    UnorderedContainer(ContainerView<'a, M>),
    Optional(OptionalView<'a, M>),
    Map(MapView<'a, M>),
    Struct(StructView<'a, M>),
    Embedded(StructView<'a, M>),
}

impl<'a, M: Default> ContainerView<'a, M> {
    pub fn item_kind(&self) -> PropertyKind;
    pub fn len(&self) -> u32;
    pub fn iter(&self) -> impl Iterator<Item = Result<ValueView<'a, M>, Error>>;
    /// O(1) for fixed-width item kinds (the offset is arithmetic); a walk otherwise.
    pub fn get(&self, index: u32) -> Result<Option<ValueView<'a, M>>, Error>;
}

impl<'a, M: Default> StructView<'a, M> {
    pub fn class_hash(&self) -> BinHash;
    pub fn properties(&self) -> impl Iterator<Item = Result<PropertyView<'a, M>, Error>>;
    pub fn property(&self, name_hash: impl Into<BinHash>)
        -> Result<Option<PropertyView<'a, M>>, Error>;
}

// MapView: key_kind() / value_kind() / len() / iter() of (ValueView, ValueView) pairs.
// OptionalView: item_kind() / get() -> Result<Option<ValueView>, Error>.
```

Because `ValueView` descends to any depth, the lazy-descent door the first draft only
reserved is now simply *open*: `Elements[3].Position` is a `ContainerView::get` and two
`StructView::property` calls, none of which materialize a sibling. The streaming
`resolve(&PropertyPath)` follow-on becomes a thin loop over this surface.

### 4.4 The object cache

Repeated lookups into one file — the bin editor chasing `ObjectLink`s, the manager resolving
the same scene objects across requests — should not re-parse. The handle holds one cache
provider behind a dyn-compatible trait; the provider *is* the policy, so there is no
separate policy enum and no third type parameter on the handle. Custom providers
(bytes-bounded, TTL, shared across handles) are the user's to write.

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

Design points:

- **`Arc<BinObject<M>>` is the currency.** A hit is an `Arc` clone: callers keep values as
  long as they like, eviction never invalidates anything, and the values cross threads.
  This is the one choice that could not be retrofitted, which is why it is fixed now even
  where the implementation waits.
- **The box requires `Send`** (`Box<dyn ObjectCache<M> + Send>`, with `M: Send` where it
  matters), so a handle with a cache installed stays `Send` for the manager's per-document
  workers. `Rc`-based providers are ruled out, deliberately.
- **`NoCache` is a real provider, not an `Option`.** The handle always holds a box; the
  default is `NoCache`, under which `cached_object()` parses on every call. One mechanism,
  no special-cased disabled state.
- **Only `cached_object()` consults it.** The cursors and `object()` never touch the cache;
  a sweep does not evict what a consumer is holding hot, and the uncached paths keep
  returning owned values as drafted.
- Dispatch is one vtable call per lookup — noise next to the parse it saves.

### 4.5 Batch lookup (added 2026-08-30)

`object(hash)` answers one question per seek; a consumer that wants fifty objects out of
one bin pays fifty seeks in whatever order it asked. `objects_batch` takes the whole
request up front so the handle can schedule the I/O:

```rust
impl<R: io::Read + io::Seek, M: Default> BinStream<R, M> {
    /// Opens the objects with the given path hashes, visiting them in file order.
    ///
    /// Takes the whole request up front so the reads can be scheduled: before the
    /// TOC exists, the requests resolve during its one forward scan of the object
    /// table, which stops as soon as every requested hash is found; with the TOC
    /// built, the requested entries are visited in offset order, so every seek is
    /// forward. Duplicate hashes in the request resolve once.
    pub fn objects_batch(
        &mut self,
        hashes: impl IntoIterator<Item = impl Into<BinHash>>,
    ) -> BatchObjects<'_, R, M>;
}

/// Lending cursor over a requested set of objects, in file order.
#[must_use = "cursors are lazy and read nothing until advanced"]
pub struct BatchObjects<'a, R: io::Read + io::Seek, M = NoMeta> { /* … */ }

impl<'a, R: io::Read + io::Seek, M: Default> BatchObjects<'a, R, M> {
    /// Advances to the next requested object the table contains.
    pub fn next(&mut self) -> Result<Option<ObjectStream<'_, R, M>>, Error>;

    /// The requested hashes the object table does not contain.
    ///
    /// Complete once `next` has returned `Ok(None)`; before that it only holds
    /// what the scan has already ruled out.
    pub fn missing(&self) -> &[BinHash];
}
```

The decisions:

- **The schedule key is the file offset, never the hash.** Hash order has no relationship
  to where objects sit in the file, so sorting a request by hash would still seek
  randomly. Offset order is what the internal buffer and the OS readahead reward — and it
  is also simply file order, which is why both the cold and the warm path can promise the
  same yield order.
- **Yield order is file order, documented.** A caller that needs request order collects
  and reorders — it has the hashes. Promising request order would force the handle back
  into random seeks and cost the whole point.
- **Cold handles finish early.** `object()` completes the full TOC scan before answering.
  A batch knows its request set, so the scan can stop at the last hit — on a request for
  objects near the front of a large bin, most of the table is never read. The rows the
  scan did pass still land in the TOC as always.
- **Misses are data, not yields.** `next` skips absent hashes; `missing()` reports them
  after exhaustion. Yielding a `None`-per-miss in file order would be unanswerable (a
  miss has no file position).
- **One open object at a time**, same lending shape as [`Objects`] and for the same
  borrow reason.

The API lands with the foundation surface but earns its keep once `view()`/`read()`
(sections 4.2–4.3) exist: descriptors alone are answered by the TOC without seeking, and
it is batch *body* reads where the monotonic schedule pays.

## 5. API surface — `PTCH`

```rust
/// A mounted `PTCH` stream: outer header, delete list, and the inner `PROP` header
/// are parsed; embedded objects and patch records stream on demand.
pub struct BinOverrideStream<R: io::Read + io::Seek, M = NoMeta> { /* … */ }

impl<R: io::Read + io::Seek, M: Default> BinOverrideStream<R, M> {
    pub fn mount(source: R) -> Result<Self, Error>;

    /// Entry hashes the layer deletes from its base bin — the outer header's
    /// `count × u32` list, mirroring `BinOverride::deleted`.
    pub fn deleted(&self) -> &[BinHash];

    // Inner-`PROP` accessors and cursors, same shape as `BinStream`:
    pub fn version(&self) -> u32;                    // inner PROP version
    pub fn class_hashes(&self) -> &[BinHash];
    pub fn objects(&mut self) -> Objects<'_, R, M>;
    pub fn entries(&mut self) -> Entries<'_, R, M>;
    pub fn toc(&mut self) -> Result<&BinToc, Error>;
    pub fn object(&mut self, path_hash: impl Into<BinHash>)
        -> Result<Option<ObjectStream<'_, R, M>>, Error>;
    pub fn cached_object(&mut self, path_hash: impl Into<BinHash>)
        -> Result<Option<Arc<BinObject<M>>>, Error>;
    pub fn set_cache(&mut self, cache: Box<dyn ObjectCache<M> + Send>);

    /// Streams the property-patch records that follow the object table.
    pub fn patches(&mut self) -> Result<Patches<'_, R, M>, Error>;

    pub fn into_bin_override(self) -> Result<BinOverride<M>, Error>;
    pub fn into_inner(self) -> R;
}

#[must_use = "cursors are lazy and read nothing until advanced"]
pub struct Patches<'a, R: io::Read + io::Seek, M = NoMeta> { /* … */ }

impl<'a, R: io::Read + io::Seek, M: Default> Patches<'a, R, M> {
    pub fn next(&mut self) -> Result<Option<PatchStream<'_, R, M>>, Error>;
}

/// One patch record: the addressing half is read, the value is not. A record is
/// self-delimiting via its payload size, so skipping is a seek.
pub struct PatchStream<'a, R: io::Read + io::Seek, M = NoMeta> { /* … */ }

impl<'a, R: io::Read + io::Seek, M: Default> PatchStream<'a, R, M> {
    pub fn object_hash(&self) -> BinHash;
    pub fn kind(&self) -> PropertyKind;
    pub fn path(&self) -> &PropertyPath;
    /// The record's value, viewed in place (the record is buffered like an object).
    pub fn value_view(&self) -> Result<ValueView<'_, M>, Error>;
    pub fn value(self) -> Result<PropertyValueEnum<M>, Error>;
    /// The fully-parsed record, as the eager reader produces.
    pub fn read(self) -> Result<PropertyPatch<M>, Error>;
}
```

For a `.bin` of unknown kind, `BinFileStream` mirrors the eager `BinFile`:

```rust
pub enum BinFileStream<R: io::Read + io::Seek, M = NoMeta> {
    Prop(BinStream<R, M>),
    Override(BinOverrideStream<R, M>),
}

impl<R: io::Read + io::Seek, M: Default> BinFileStream<R, M> {
    pub fn mount(source: R) -> Result<Self, Error>;
}
```

## 6. Skip semantics

Skipping mirrors `MetaValue_skipByType`: primitives by fixed width, strings by length
prefix, complex values (`Container`, `UnorderedContainer`, `Struct`, `Embedded`, `Map`) by
their stored byte size, `Optional` by recursing into its zero-or-one element, `BitBool` as
one byte. Whole objects and whole patch records skip by their own size fields. No skip
allocates or decodes value contents.

At the file level a skip is a seek; inside a buffered object it is slice arithmetic over
the view. Same rules, two costs, one implementation in the wire core (section 9).

## 7. Strictness: counts drive the parse, sizes drive the skips, disagreement is an error

*(Revised 2026-08-30: the first draft recorded size mismatches in a tolerant side-channel
log — `discrepancies()` / `discrepancy_count()` / `SizeDiscrepancy` — on the "mirror the
client" rationale. That was the wrong frame: a declared size that disagrees with the
count-driven walk means the file's skip path and parse path no longer describe the same
bytes, which is exactly when continuing is unsafe — the TOC and every `byte_range` are
built from sizes the parse just proved wrong. The corpus attests shipped files never
exhibit the mismatch, so the tolerance bought complexity and a silent-corruption hazard
in exchange for nothing. The log is gone; the walk raises.)*

The two paths and their trust model:

- **Skip path**: the declared size is the seek distance. There is nothing else to trust,
  and a value the parse path would reject still skips cleanly by its size — which is
  also what the client does with it.
- **Parse path**: counts drive the walk over the buffered bytes, exactly as they drive
  the client's parser. A sized region's declared size is compared against what the
  counts consumed after the fact, and a disagreement is `Error::InvalidSize(declared,
  consumed)` — the same variant the eager readers have always raised for this condition.
  One condition, one error, on both paths.

That unification is what keeps `Bin::from_reader`'s behavior unchanged when it is rebuilt
over the stream (section 9): the inline checks in the `ReadProperty` impls and the walk's
check are the same check raising the same error. The homogeneity failures stay where they
are, hard errors from the value model's checked constructors (`InvalidNesting`,
`InvalidKeyType`, `MismatchedContainerTypes`).

A consumer surveying broken or hand-crafted files catches the error per chunk — tooling
built on the error, not state built into the core. After a mismatch the handle's
sequential sweep is not trustworthy (the mismatch is the proof of that); random access
through the already-harvested TOC rows remains valid, since those offsets tiled correctly
up to the failure.

## 8. The legacy-numbering latch

The eager reader detects legacy property-kind numbering by failing on a kind byte and
re-reading the whole object table with the legacy mapping. The stream latches instead:

- The handle starts in current numbering.
- When decoding a kind byte fails (`Error::InvalidPropertyTypePrimitive`) during any parse
  or skip, the current *object* is re-read from its own start in legacy numbering. If that
  succeeds, the handle latches legacy for the rest of its life. With buffered objects the
  retry is a re-walk of bytes already in memory — no I/O. A view captures the flag at
  creation, so a view handed out before the latch keeps the numbering it was built under.
- Objects yielded before the latch are not revisited; a streaming consumer that already
  acted on them acted on data parsed under the wrong mapping only if those objects happened
  to parse cleanly both ways — the same ambiguity the eager retry has, narrowed to a prefix.
- `into_bin()` removes the asymmetry: on latch it restarts the drain from the top of the
  object table, reproducing the eager reader's behavior exactly.

As today, the retry can reinterpret a genuinely desynced file as "legacy"; the latch does
not widen that hazard, and a latched handle reports it (`fn is_legacy(&self) -> bool`).

Mechanically the latch is nothing new: `Kind::unpack(raw, legacy)` already centralizes the
legacy fudging for every kind byte in the crate, and the latch is simply the `legacy`
argument the stream feeds it (and `read_property_kind`) from handle state instead of from
a function parameter.

## 9. Layering: one wire core under two renderers

With views in the design there are two ways to decode a value — borrowed from bytes, and
owned into `PropertyValueEnum`. The single-decode-path rule therefore moves down one
level: **one module owns the wire** — offsets, headers, `ValueShape`, skip distances, and
the leaf codecs over `&[u8]` — and both surfaces are renderers over it:

```text
        wire core (offsets, shape, skip, leaf codecs over &[u8])
         /                         \
  ReadProperty impls          ObjectView / ValueView
  (owned PropertyValueEnum)   (borrowed, zero-copy)
         \                         /
     into_bin() == from_reader   corpus parity sweep
```

`Bin::from_reader` becomes:

```rust
pub fn from_reader<R: io::Read + io::Seek + ?Sized>(reader: &mut R) -> Result<Self, Error> {
    BinStream::mount(&mut *reader)?.into_bin()
}
```

(with `BinStream` implemented over `R: Read + Seek + ?Sized`-friendly internals, or a thin
`&mut R` shim — implementation detail). `BinOverride::from_reader` likewise drains a
`BinOverrideStream`. The `ReadProperty` impls are rebuilt over the wire core's codecs
rather than reading `io::Read` directly; the top-level loops exist once, in the stream.

Two refactors this forces, visible in #187's code:

- The `ReadProperty` impls verify sizes *inline* today (`Container::from_reader` measures
  its body and returns `Error::InvalidSize` itself). Under section 7 that check moves
  into the wire core's walk, which raises the same `Error::InvalidSize` — one check, one
  place. The homogeneity checks (`InvalidNesting`, `InvalidKeyType`,
  `MismatchedContainerTypes`) stay in the value model — they are model invariants, not
  stream policy — and the views surface the same errors from the same core.
- Leaf decoding moves from `io::Read` methods to `&[u8]` codecs, with the eager path
  feeding them from its buffer. That is what guarantees a `ValueView::String` and an
  owned `values::String` can never disagree about the same bytes.

`corpus.rs` (#187's harness, gated on `LTK_LOL_GAME_DIR`) grows the stream parity sweep:
for every `PROP` and `PTCH` chunk in an install, (a) `entries()` harvests the same
`(path, class)` set the eager parse holds, (b) `into_bin()` equals `Bin::from_reader`,
(c) `object(hash)` on a sample equals the eager lookup, and (d) every declared size equals
`PropertyExt::size` over the parsed values — attesting that shipped files are size-clean,
not just parse-clean.

## 10. What this deliberately does not do (yet)

- **Streaming `resolve`.** Lazy descent itself is no longer deferred — `ValueView`
  delivers it (section 4.3) — but `ObjectStream::resolve(&PropertyPath)`, the loop that
  walks a path over the views with the resolver's traversal and type rules
  ([#187](https://github.com/LeagueToolkit/league-toolkit/pull/187)), stays a *named
  follow-on issue*: it is now thin, and thin is exactly when it should wait for a consumer.
- **Writing, in v1.** The stream itself stays read-only. The write-back *contract* — a
  delta rewrite of a whole `.bin` by copy-through of untouched objects' raw bytes plus
  re-serialization of edited ones — is now specified in section 15, because the bin
  editor's flow depends on its shape; its implementation is still a stage after v1.
- **Parallel access.** One cursor at a time per handle, `&mut self` throughout. The
  fan-out workloads parallelize per file, not within one.
- **Caching by default.** `object()` and the cursors parse on every call and return owned
  data. Caching exists only as the opt-in `cached_object()` path through the installed
  [`ObjectCache`] provider (section 4.4), whose implementation may trail v1 if it does not
  stay a small self-contained addition — the API shape is committed either way. A consumer
  that wants a resident tree uses `into_bin()`.

## 11. Resolutions (design review, 2026-08-30)

Every open question from the first draft was settled in the follow-up review round, along
with the decisions the review added. The surface in sections 4–7 reflects all of them.
(A later round revised the per-object layer itself — section 14 — which supersedes the
rows here that speak of property cursors and the reserved lazy-descent door.)

| Question                                  | Resolution                                                                                                                                                                                                                                                          |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `M` on methods or on the handle?          | On the handle: `BinStream<R, M = NoMeta>` with `concrete` aliases. `concrete` can only pin type-position generics, so method-level `M` would force a turbofish at every call site.                                                                                  |
| Buffering                                 | Internal `BufReader` + `seek_relative`; callers hand over the bare source, `into_inner()` unwraps. A caller-supplied `BufReader` is worse than nothing (plain `seek` discards its buffer), and the type system cannot catch that — so the handle owns the problem.  |
| `Entries` without `&mut`?                 | Already answered by `toc()`: `&BinToc` iterates shared once built, and `BinToc: Clone` lets a consumer detach it. The streaming `entries()` keeps its value (early exit without the full sweep).                                                                    |
| `SizeDiscrepancy` bounding                | Superseded (revision, 2026-08-30): the discrepancy log was removed entirely — a size that disagrees with the count-driven walk is `Error::InvalidSize` (section 7), so there is nothing to bound.                                                                    |
| `read()` vs `parse()`                     | `read()` — it does I/O, and the crate's vocabulary is `from_reader` / `ReadProperty`; `parse` appears nowhere in the API today.                                                                                                                                     |
| Streaming `resolve(&PropertyPath)` in v1? | No — named follow-on issue; `value_range()` is the v1 commitment (section 10).                                                                                                                                                                                      |
| Cursor restart semantics                  | `objects()` / `entries()` always restart from the top of the table. Idempotent, no hidden state; resumption is what the TOC and `object(hash)` are for.                                                                                                             |
| New `Error` variants are breaking         | `Error` gains `#[non_exhaustive]` in the 0.8.0 release #187 opens — the free breaking moment — so this and later work grow variants in minors.                                                                                                                      |
| Lookup caching (added in review)          | Designed now (section 4.4), implemented in v1 only if it stays cheap. `Box<dyn ObjectCache<M> + Send>` on the handle, `NoCache` as the default real provider, `LruObjectCache` shipped, `Arc<BinObject<M>>` returns — the one choice that could not be retrofitted. |
| PTCH outer-header round-trip              | Answered by #187's `corpus.rs`: the outer header, delete list included, already round-trips byte-exactly (section 12).                                                                                                                                              |

## 12. Interaction with #187

[#187](https://github.com/LeagueToolkit/league-toolkit/pull/187) (`feat/ptch-resolve`) is open,
currently behind `main`, and rewrites the value model this design reads into. **It should land
first.** It rewrites `values/container.rs`, `values/optional.rs`, `values/map.rs` and
`property/enum.rs` - the same `ReadProperty` impls section 6's skip functions have to sit beside -
so a stream built on the pre-#187 model gets rewritten after it merges. Three of its eight commits
are breaking (`refactor(meta)!`, two `feat(meta)!`), which puts `ltk_meta` at 0.8.0; there is no
reason to spend a second breaking bump on the same crate a release later.

What it changes for this design:

- **The metadata parameter needs revisiting.** #187 added `ltk_meta::concrete`, a module of
  `M = NoMeta` aliases, for a reason its own docs state plainly: Rust applies a type parameter's
  default in type position but never in expression position, so a generic name in a `let` needs an
  annotation or a turbofish. Section 4 puts `M` on the value-producing methods rather than the
  handle, which is exactly the position where no default applies - every `into_bin()`, `read()`,
  `value()` and `property()` call site would need `::<NoMeta>` or an annotated binding. Either
  `concrete` grows stream aliases too, or `BinStream<R, M = NoMeta>` moves the parameter to the
  handle where the default does apply. Section 2 settled the handle question before `concrete`
  existed, so this was a decision to revisit rather than one already made. **Revisited and
  resolved in the 2026-08-30 review: the parameter moves to the handle** (sections 4 and 11),
  and `concrete` grows the three stream aliases.
- **The value model flattened.** `Container`, `UnorderedContainer` and `Optional` held typed
  variants; they now hold `item_kind: Kind` beside `Vec<PropertyValueEnum<M>>` (`Optional` boxes
  its single value). The wire format is untouched, so section 6 stands unchanged. The parse path
  is what moves: the checked constructors and `push` enforce homogeneity at run time and can fail
  with `Error::InvalidNesting` or `Error::MismatchedContainerTypes`, so the streaming parser either
  goes through those same checks or becomes a second, unchecked way to build a container. Section
  9's single-decode-path rule is what keeps that from happening.
- **No `&mut PropertyValueEnum` is handed out any more.** `ValueSlot` replaced it, carrying the
  kind its holder pins the value to. Section 4.2's choice to return owned values from `property()`
  and `read()` agrees with that direction, and the write stage in section 10 inherits `ValueSlot`
  as the mutation surface rather than having to invent one.
- **The path resolver ships in #187, not later.** `PropertyPath`, `Bin::resolve`, `resolve_mut`,
  `patch` and the `ValueShape` type rule all exist there, checked against 23,047 shipped records
  across 456 archives. The reserved door in section 10 is still the right shape, but what it opens
  onto is a resolver whose traversal rules are already written down and corpus-tested: a streaming
  `ObjectStream::resolve(&PropertyPath)` that descends only the properties on the path is an
  obvious follow-on rather than a speculative one.
- **`PatchStream::path()` returns `&PropertyPath`.** `PropertyPatch::path` is already a
  `PropertyPath`, so `&str` would make the streaming surface strictly weaker and force callers to
  re-parse. Corrected in section 5.
- **`corpus.rs` is the harness section 9 assumes.** #187 replaced the phase-1 scratch tooling with
  `crates/ltk_meta/tests/corpus.rs`, ignored unless `LTK_LOL_GAME_DIR` points at an install. It
  already sweeps every archive in a live install and compares re-written `PTCH` chunks byte for
  byte (238 of 238), which is the shape "exercise the stream on every corpus file for free" needs.
  It also answers what was an open question here: the `PTCH` outer header, delete list included,
  already round-trips byte-exactly.

Design conventions worth borrowing from `ptch-property-patches.md`, which #187 implements: numbered
decisions each recording the alternative not taken, and a closing implementation-notes section that
records where reading the client reference again corrected the design (its section 15.3 corrected
D11 after the fact). This document's section 2 is the decision table; it has no equivalent of the
correction log yet.

## 13. Taken from #187's code (review, 2026-08-30)

A review of the branch itself, beyond what section 12 already covers, adopted four things
into this design and raised one action item on the PR:

- **`ValueShape` is the streaming peek** (section 4.3). The resolver's type-rule descriptor
  — kind, item kind, key kind, embed class — is exactly what a complex value's wire header
  carries ahead of its body, so `PropertyStream::shape()` returns it (filled by
  `ValueShape::of`'s rules) at skip cost. One vocabulary for "what is this value" across
  the resolver, the patch checker and the stream; `item_count()` rides in the same bytes.
- **`Kind::unpack(raw, legacy)` is the latch** (section 8). The legacy fudging is already
  centralized; the stream feeds the flag from handle state.
- **`corpus.rs` is where stream parity lives** (section 9): the harness already sweeps an
  install; the stream adds its equivalence checks there rather than growing a second one.
  `PropertyExt::size(include_header)` — values computing their own serialized width — is
  additionally the natural debug-assert cross-check for skip distances, and later the
  measuring half of the rebase/write stage.

Also noted, not adopted: the inline `InvalidSize` checks inside the `ReadProperty` impls
must move into the stream's measuring layer (section 9), and the homogeneity errors stay
where they are (section 7).

**Action item on #187 while its breaking window is open:** the `Error` enum is still
exhaustive on the branch. Section 2's error-surface decision (`#[non_exhaustive]` in 0.8.0)
belongs *in that PR*, not in the stream's later one — and its new public enums
(`ResolveErrorKind`, `PatchError`) are candidates for the same attribute at the same
moment, which is the maintainer's call.

## 14. Revision: buffered object views (2026-08-30)

The per-object layer was revised in a second design round, after measuring the owned value
model (`PropertyValueEnum` is 96 bytes per node, align 16 — a wire `f32` costs 96 bytes
materialized). The first draft streamed *properties* through forward-only lending cursors
over the reader; the revision buffers one object's declared byte range into a handle-owned
reused buffer and views it zero-copy (sections 4.2–4.3).

What changed and why:

- **Cursors → views.** The lending cursors existed to avoid materializing anything, and
  paid for it: no `std` iterators, one property at a time, no backtracking. An object's
  size is known before descending and objects are KB-scale, so buffering one object keeps
  the memory bound tight while dissolving every one of those restrictions. The *file*
  level is untouched — the object-table sweep still streams and skips by size, which is
  where the constant-memory guarantee actually matters.
- **`ValueView` replaces the reserved door.** The first draft deferred lazy descent and
  reserved `value_range()`; the borrowed mirror of `PropertyValueEnum` makes descent to
  any depth the natural surface instead, and a read-only consumer now materializes
  nothing at all — the 96-byte node cost is simply not paid.
- **The single-decode-path rule moved down a level** (section 9): one wire core over
  `&[u8]`, with the owned `ReadProperty` impls and the views as its two renderers.
- **Strictness got simpler** (section 7): buffering by declared size means skip, view and
  parse all land on the same next-offset, so the skip-versus-parse divergence the first
  draft documented as a hazard no longer exists in our reader — a walk that disagrees
  with its declared size surfaces as `Error::InvalidSize` instead of desyncing anything.

Alternatives not taken: keeping both cursor and view APIs (two per-object models to keep
in agreement, and views subsume the cursor's uses); typed accessors without a `ValueView`
enum (stops one level down); views delegating to the existing `io::Read`-based readers
through `io::Cursor` (owned allocation per leaf — zero-copy in name only).

Cost accepted: a pathological file whose single object is enormous buffers that object.
The eager reader materializes a multiple of the same bytes, so this is still the smaller
footprint everywhere it matters.

## 15. The write-back contract: delta rewrite of a `.bin`

Added for the bin editor's flow (expand → lazily read one object → edit → save). The save
target is a **rewritten `.bin` file**. Authoring the edit as a `PTCH` layer is explicitly
out of scope: that layer is not yet designed for mods, and nothing here forecloses it —
a delta is upstream of either output form.

This section fixes the *contract*; the implementation is a stage after v1 (section 10).

### 15.1 The editor flow over this API

1. **Expand / navigate**: `mount()` + `toc()` give the object rows (path, class, size)
   without parsing; browsing an object read-only is `object(hash)` → `view()`.
2. **Read for editing**: `object(hash)?.read()` → an owned `BinObject<M>`. The editing
   path takes `read()`, never `cached_object()` — the cache hands out shared `Arc`s, and
   an edit wants exclusive ownership. (Cache for viewing, `read()` for editing;
   `Arc::make_mut` is the escape hatch when both are wanted.)
3. **Edit**: #187's mutation surface — `resolve_mut(&PropertyPath)` → `ValueSlot` →
   `ValueMut`, or structured operations that are shape-for-shape the editor's patch table.
   The edited object goes into the document's delta; undo is inverse patches over it.
4. **Save**: `write_patched` to a temp file, rename over. After a rename-over, the mounted
   handle still describes the *old* bytes — the consumer remounts.

### 15.2 The types

```rust
/// Edits held against a mounted base. Costs O(edited objects), not O(file).
#[derive(Debug, Default, Clone)]
pub struct BinDelta<M = NoMeta> {
    /// Objects to write in place of the base's, keyed by path hash.
    replaced: IndexMap<BinHash, BinObject<M>>,
    /// Base objects to drop.
    removed: HashSet<BinHash>,
    /// New objects, appended after the base's in file order.
    appended: Vec<BinObject<M>>,
    /// `None` keeps the base's dependency list.
    dependencies: Option<Vec<String>>,
}

impl<R: io::Read + io::Seek, M: Default + Clone> BinStream<R, M> {
    /// Writes the base with `delta` applied.
    ///
    /// Header and class table are rebuilt for the final entry set; every untouched
    /// object is raw-copied **byte for byte** from its [`ObjectEntry`] range; replaced
    /// and appended objects are serialized through the eager writer. Entry order is the
    /// base's file order, minus `removed`, with `replaced` in place and `appended` last.
    pub fn write_patched<W: io::Write>(&mut self, delta: &BinDelta<M>, out: &mut W)
        -> Result<(), Error>;
}
```

### 15.3 Invariants

- **Untouched means bit-identical.** An object the delta does not name is never
  deserialized — its bytes are copied from `byte_range()`. This is a stronger guarantee
  than the editor's current "the backend owns the tree" model: a kind with no widget, a
  hash no table names, a container order, a duplicate key — none of it can be lost,
  because none of it is interpreted.
- **The version passes through.** The header writes the version that was read, so saving
  one edit does not upgrade the file — the version-3-rewrite hazard the editor documents
  applies only to objects that were actually edited (which re-encode through the current
  writer), not to the file.
- **A legacy-latched base refuses the delta write.** Raw-copied objects would keep the
  legacy kind numbering while re-encoded ones wrote modern numbering — a mixed, corrupt
  file. `is_legacy()` handles get a dedicated error; the consumer falls back to a full
  `into_bin()` + `to_writer` transcode, or opens read-only. Shipped files are modern, so
  this is a guard, not a path.
- **Size mismatches cannot reach this path.** Raw copy-through never walks an unedited
  object, so a lying size field in one is copied exactly as its declared range states,
  reproducing the input byte for byte. An *edited* object was necessarily read, and a
  size mismatch there already failed the read with `Error::InvalidSize` (section 7).

### 15.4 What this deliberately is not

- **Not `PTCH` authoring.** A `BinDelta` could later *also* render as patch records —
  the operations are shape-compatible — but that output form waits until the patch layer
  is designed for mods.
- **Not a mutable view.** No `ValueViewMut` over the buffered bytes: in-place byte
  mutation only works for fixed-width leaves (a string edit shifts every size field above
  it), so owned-`BinObject`-per-edited-object is the right granularity, and #187's
  `ValueSlot` already guards mutation on the owned side.
- **Not an in-place file update.** The write always produces a complete new stream
  (temp + rename at the consumer's discretion); offsets shift freely and nothing is
  patched into the middle of a file.
