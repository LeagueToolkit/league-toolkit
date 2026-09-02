# Streaming bin reading in `ltk_meta`

The API spec for `ltk_meta::stream`: mounting a bin, sweeping its object table, viewing one object
without materializing it, and the delta rewrite that saves an edit.

**This document states what is true now.** Where the code and a section here disagree, the section
is the bug and gets edited. Two things it does not hold:

- **Why streaming exists, who asks for it, and what it must do** -
  `docs/prd/002-streaming-bin-reading.md`, cited here as FR-N.
- **Why an option was chosen over the alternatives it beat** - `docs/adr/0007` to `0011`, cited as
  ADR-NNNN from the rules in [section 13](#s13).

Implemented today: the `PROP` half - the foundation, the buffered views, the owned decode with its
lookup cache, and batch lookup (#207, #208, #209, #214). Still to come: the `PTCH` stream of
[section 5](#s5) (#210) and the delta write-back of [section 10](#s10) (#211).

## <a id="s1"></a>1. Summary

`Bin::from_reader` parses every property of every object into an owned tree. `ltk_meta::stream` is
the reader for everything else: a header without a body, a path-hash harvest across 42,306 files,
one object out of a file on demand.

The client's own loader is the model. `MetaFile_readEntry` walks the object table front to back,
deserializes each sized entry as it arrives, and uses the size fields only to seek past what it will
not parse. It never builds a whole-file tree. Streaming is the format's canonical reading model and
the eager `Bin` is the derived convenience, so the crate is layered that way round: the stream is
the only parser, and `Bin::from_reader` is mount plus drain (ADR-0008).

The module holds:

- **`BinStream<R: Read + Seek>`** - an owning handle over a `PROP` stream, mounted the way
  `ltk_wad::Wad` is. Mounting reads the header, dependencies and class-hash table - all sequential,
  no seeking - and stops.
- **A cursor at the file level, zero-copy views at the object level.** `objects()` sweeps the object
  table yielding one `ObjectStream` at a time; what is not descended into is skipped by size,
  exactly as the client skips. Descending buffers the object's declared byte range into a reused
  buffer and hands back an `ObjectView`: `std` iterators over properties, a borrowed `ValueView`
  mirror of `PropertyValueEnum` to any depth, nothing materialized until asked for (ADR-0007).
  Owning is one call away - `read()` to a `BinObject`, `value()` to a `PropertyValueEnum`.
- **A plain-data layer over the cursors.** `entries()` is a `std::iter::Iterator` of `ObjectEntry`
  descriptors for harvesting; `object(path_hash)` gives random access, building the offset table
  transparently on first use; `objects_batch` takes a whole request up front so the reads can be
  scheduled in file order.
- **Opt-in lookup caching.** The handle holds a cache provider - `NoCache` by default - and
  `cached_object()` hands out `Arc<BinObject<M>>`, so a consumer resolving the same objects
  repeatedly pays each parse once (ADR-0011).
- **`BinOverrideStream<R>`** - the same treatment for `PTCH` files, including the patch records the
  eager reader also parses and the outer header's delete list.

What this deliberately does not do is [section 11](#s11); the requirements behind all of it are
PRD-002.

## <a id="s2"></a>2. Vocabulary

Every term this document uses in a specific sense.

**Opening and moving through a file**

- **mount** - open a handle over a source: read the header, the dependencies and the class-hash
  table, then stop. The name matches `ltk_wad::Wad::mount`, and it means the same thing.
- **sweep** - a front-to-back pass over the object table. Every sweep restarts from the top and
  holds no state between calls; resuming is what the TOC and `object(hash)` are for.
- **descend** - to buffer one object's declared byte range and look inside it. The file level
  streams and skips; the object level does not (ADR-0007).
- **cursor** - a lending cursor at the file level: `Objects`, `BatchObjects`. Not a `std` iterator,
  because each yielded item borrows the reader, which is what enforces one open object at a time.
  There are no cursors inside an object.
- **view** - a borrowed, lazily-decoded window over an object's buffered bytes: `ObjectView`,
  `PropertyView`, `ValueView` and their kind-specific relatives. Nothing decodes until touched,
  nothing allocates until an owned value is asked for.

**Bytes**

- **skip** - advance past a value by the size its header declares, decoding nothing. At the file
  level a seek; inside a buffered object, slice arithmetic.
- **walk** - cross a value driven by its counts, the way the client's parser does. A declared size
  that disagrees with what a walk consumed is an error, not a discrepancy (ADR-0009).
- **layout core** - `stream::layout`, crate-internal: where a value starts, how far it runs, what
  its header declares, and the leaf codecs over `&[u8]`. It is called the layout core and not "the
  wire" because which side of the I/O boundary the bytes are on is the least interesting thing
  about it.
- **renderer** - one of the two surfaces built over the layout core: the owned `ReadProperty`
  impls, and the views. Neither decodes anything the core does not (ADR-0008).

**Facts about a file**

- **entry** - one object's `(path_hash, class_hash, offset, size)`, as `ObjectEntry`. The **TOC**
  (`BinToc`) is the file's entries plus a hash index: plain data, cloneable, detachable from the
  handle, serializable behind the `serde` feature.
- **numbering** - which property-kind mapping a file's bytes were written in, current or legacy.
  `Numbering` travels with the bytes it describes; a cursor carries one rather than taking a
  `legacy: bool` per operation.
- **latch** - the handle settling on legacy numbering for the rest of its life, after a kind byte
  fails to decode and the object re-walks cleanly the other way ([section 8](#s8)).

**Writing**

- **delta rewrite** - the write-back shape: a whole `.bin` rewritten, with untouched objects copied
  through byte-exactly and only edited ones re-encoded ([section 10](#s10)).

## <a id="s3"></a>3. Wire facts the design leans on

- **The class table is free, path hashes are not.** The object table is
  `u32 count`, then `count x u32 class_hash`, then `count x (u32 size, u32 path_hash,
  u16 prop_count, properties...)`. After the sequential header read the handle already holds
  every class hash. Harvesting path hashes takes one seek-hop per object reading 8 bytes
  (`size`, `path_hash`), which is also the moment each object's `(offset, size)` is learned -
  so the sweep that harvests is the sweep that indexes.
- **Every complex value carries its byte size.** Objects, `Struct`/`Embedded`, containers and
  maps store a size ahead of their body; primitives have fixed widths and strings a length
  prefix. Skipping any unparsed value is therefore a seek, mirroring `MetaValue_skipByType`.
- **The client never verifies sizes on the parse path.** It trusts counts when parsing and
  reads sizes only to skip. `ltk_meta`'s eager reader measures every region and errors on
  mismatch. The stream takes the client's semantics ([section 7](#s7)).
- **Legacy property-kind numbering is detectable only by parsing.** The eager reader retries
  the whole object table in legacy numbering when a kind byte fails to decode. A streaming
  reader discovers this mid-sweep; [section 8](#s8) defines the latch.

## <a id="s4"></a>4. API surface - `PROP`

All types live in `ltk_meta::stream` and are re-exported in full from the crate root, which is
where a consumer names them. Signatures are the design; doc comments are abbreviated. `M` is the
same property-meta parameter the eager types carry, defaulting to `NoMeta`, and lives on the handle
(ADR-0010) - the only placement a `concrete` alias can reach. The alias is what removes the
annotation, because `mount` is itself expression position and Rust applies no default there:
`concrete::BinStream::mount(file)` pins `M`, which then disappears from every downstream signature.
So `concrete` carries the expression-position names of the shipped surface - `BinStream`,
`LruObjectCache` and its `NoCache` sibling - while every other type is named in type position,
where the default applies. The `PTCH` handles of [section 5](#s5) want the same alias when they
land.

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

    // -- header facts, free after mount --------------------------------------
    pub fn version(&self) -> u32;
    pub fn dependencies(&self) -> &[String];
    /// Class hash of every object, in file order. `class_hashes().len()` is the object count.
    pub fn class_hashes(&self) -> &[BinHash];

    // -- sweeping ------------------------------------------------------------
    /// A cursor over the object table. Every call starts a fresh sweep from the top;
    /// cursors hold no state between calls. Objects not descended into are skipped
    /// by their size field.
    pub fn objects(&mut self) -> Objects<'_, R, M>;

    /// A `std` iterator of plain descriptors, for harvesting and filtering.
    /// Equivalent to `objects()` without ever descending; restarts the same way.
    pub fn entries(&mut self) -> Entries<'_, R, M>;

    // -- random access -------------------------------------------------------
    /// The table of contents: every object's `(path_hash, class_hash, offset, size)`.
    ///
    /// Built by one harvest sweep on first use, then cached. `objects()` / `entries()`
    /// sweeps also populate it as a side effect, so a fully-swept handle pays nothing.
    pub fn toc(&mut self) -> Result<&BinToc, Error>;

    /// Opens the object with the given path hash, building the TOC if needed.
    pub fn object(&mut self, path_hash: impl Into<BinHash>)
        -> Result<Option<ObjectStream<'_, R, M>>, Error>;

    // -- cached lookup (section 4.4) -----------------------------------------
    /// Resolves an object through the installed [`ObjectCache`]: a hit is an `Arc`
    /// clone with no I/O, a miss parses and inserts. Under the default [`NoCache`]
    /// this parses on every call.
    pub fn cached_object(&mut self, path_hash: impl Into<BinHash>)
        -> Result<Option<Arc<BinObject<M>>>, Error>;

    /// Installs a cache provider. The default is [`NoCache`].
    pub fn set_cache(&mut self, cache: Box<dyn ObjectCache<M> + Send>);

    // -- upgrade / teardown --------------------------------------------------
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
    /// The entry with the largest declared size, or `None` for an empty file.
    ///
    /// Known before any object body is decoded: `toc()` is one seek-hop per object and reads
    /// nothing past the 8-byte header. It bounds what a sweep that reads one object at a time
    /// can cost - the bytes of the file plus this object's expansion - which is the number a
    /// consumer budgeting a streamed read wants (S23).
    pub fn largest(&self) -> Option<&ObjectEntry>;
}
```

### <a id="s4.1"></a>4.1 The object cursor

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

/// `std` iterator of plain [`ObjectEntry`] descriptors - `Objects` without descent.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct Entries<'a, R: io::Read + io::Seek, M = NoMeta> { /* Objects<'a, R, M> */ }

impl<R: io::Read + io::Seek, M: Default> Iterator for Entries<'_, R, M> {
    type Item = Result<ObjectEntry, Error>;
    fn next(&mut self) -> Option<Self::Item>;
}
```

### <a id="s4.2"></a>4.2 One object

```rust
/// A view of one object positioned in the stream. Dropping it without descending
/// costs nothing; the parent cursor skips by size.
pub struct ObjectStream<'a, R: io::Read + io::Seek, M = NoMeta> { /* ... */ }

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
    /// buffer) and returns a zero-copy view over it. Everything inside the object -
    /// iteration, random access, descent - happens in memory from here (section 4.3).
    ///
    /// The object is walked once as it lands. The numbering latch has to be settled
    /// before any view exists (section 8), and that walk is where [`Error::InvalidSize`]
    /// is raised (section 7) - so a view is only ever handed out over bytes whose
    /// declared sizes and property counts already agree, and the lazy surfaces inside
    /// it need no size checks of their own.
    pub fn view(&mut self) -> Result<ObjectView<'_, M>, Error>;

    /// Parses the whole object into an eager [`BinObject`]. (`read`, not `parse`:
    /// it does I/O, and the crate's vocabulary is `from_reader` / `ReadProperty`.)
    ///
    /// Decodes directly, without the walk `view()` performs: a count-driven decode
    /// already crosses the same sized regions and raises the same errors, so an object's
    /// bytes are crossed once, not twice.
    pub fn read(&mut self) -> Result<BinObject<M>, Error>;
}
```

### <a id="s4.3"></a>4.3 Views

The per-object layer is zero-copy views over the buffered bytes, not cursors over the
reader. Views are plain shared references: `std` iterators, any number of properties held
and compared at once, backtracking free, skipping is slice arithmetic. Nothing decodes
until touched, and nothing allocates until an *owned* value is asked for. The views carry
the handle's `M` as a phantom parameter so the owned-decode escape hatches infer without
turbofish; the borrowed data itself is metadata-free.

```rust
/// One object's bytes, viewed in place.
pub struct ObjectView<'a, M = NoMeta> { /* path/class hashes, &'a [u8], Numbering */ }

impl<'a, M: Default> ObjectView<'a, M> {
    pub fn path_hash(&self) -> BinHash;
    pub fn class_hash(&self) -> BinHash;
    /// The wire count. `BinObject::properties` is keyed and so deduplicated; see
    /// `property` for when the two can differ.
    pub fn property_count(&self) -> u16;

    /// The properties in file order. A real `std` iterator; items are `Result` because
    /// a header's kind byte can fail to decode.
    pub fn properties(&self) -> Properties<'a, M>;

    /// Random access by name hash - an in-memory walk, no index needed. Returns the
    /// **first** property with that hash and stops there; the owned side's keyed map
    /// keeps the last. The two differ only for an object declaring one name hash twice,
    /// which no shipped bin does, and closing the gap would cost every lookup its early exit.
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

    /// The value's wire shape - a container's item kind, a map's key and value kinds,
    /// an embed's class - from the few header bytes ahead of the body. Returns the
    /// same [`ValueShape`] the resolver's type rule uses
    /// (`ptch-property-patches.md` section 9.3), filled by the rules
    /// of `ValueShape::of` (a pointer's class is not recorded).
    pub fn shape(&self) -> Result<ValueShape, Error>;

    /// For containers and maps, the element count from the same header bytes. `None`
    /// for every other kind, an option included - whether an option holds anything is
    /// `OptionalView::is_some`.
    pub fn item_count(&self) -> Result<Option<u32>, Error>;

    /// The value's raw bytes (header excluded).
    pub fn raw(&self) -> &'a [u8];

    /// Descends into the value without materializing it.
    pub fn value_view(&self) -> Result<ValueView<'a, M>, Error>;

    /// Decodes the value - the whole subtree - into the existing owned representation.
    pub fn value(&self) -> Result<PropertyValueEnum<M>, Error>;
}
```

`ValueView` is the borrowed mirror of `PropertyValueEnum`, one variant per `Kind`, the
same shape `ValueMut` takes for mutation:

```rust
/// A borrowed, lazily-decoded value. Leaves carry decoded primitives (`&'a str` for
/// strings, validated on access); complex kinds carry sub-views that descend further,
/// still zero-copy, to any depth.
pub enum ValueView<'a, M = NoMeta> {
    None,
    Bool(bool),
    I8(i8), /* ... */ U64(u64), F32(f32),
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
    pub fn iter(&self) -> ContainerItems<'a, M>;
    /// O(1) for fixed-width item kinds (the offset is arithmetic); a walk otherwise.
    pub fn get(&self, index: u32) -> Result<Option<ValueView<'a, M>>, Error>;
}

impl<'a, M: Default> StructView<'a, M> {
    pub fn class_hash(&self) -> BinHash;
    pub fn properties(&self) -> Properties<'a, M>;
pub fn property(&self, name_hash: impl Into<BinHash>)
        -> Result<Option<PropertyView<'a, M>>, Error>;
}

// MapView: key_kind() / value_kind() / len() / iter() -> MapEntries<'a, M> of (ValueView, ValueView) pairs.
// OptionalView: item_kind() / get() -> Result<Option<ValueView>, Error>.
```

Because `ValueView` descends to any depth, `Elements[3].Position` is a `ContainerView::get` and two
`StructView::property` calls, none of which materialize a sibling. The views are one of the two
trees the visitor walk runs over: `ObjectView::walk`, `ObjectStream::walk` and `BinStream::walk`
are specified in `value-walk.md` [section 5](value-walk.md#s5) (S25). The streaming
`resolve(&PropertyPath)` follow-on ([section 11](#s11)) is a thin loop over this surface.

Two shapes the whole view family shares. **The iterators are named types** - `Properties`,
`ContainerItems`, `MapEntries` - rather than `impl Iterator`: a returned `impl Iterator` cannot be
named by a caller storing one, and it would have had to spell out its lifetime capture anyway. The
named types also carry `Debug`, `Clone`, `FusedIterator` and an exact `size_hint`. And **every view
is `Copy` for every `M`**, with `Clone`, `Copy` and `Debug` written by hand: `M` is a phantom, so a
derive would demand `M: Copy` for a field that holds nothing.

### <a id="s4.4"></a>4.4 The object cache

Repeated lookups into one file - the bin editor chasing `ObjectLink`s, the manager resolving
the same scene objects across requests - should not re-parse. The handle holds one cache
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

/// Least-recently-used cache bounded by object count. Carries `M`, because it holds
/// `Arc<BinObject<M>>`.
///
/// Recency is the map's own order, which makes an access `O(capacity)` in bookkeeping -
/// noise beside the parse it saves, at the sizes this is for.
#[derive(Debug)]
pub struct LruObjectCache<M = NoMeta> { /* capacity: NonZeroUsize, ... */ }

impl<M> LruObjectCache<M> {
    pub fn new(capacity: NonZeroUsize) -> Self;
}
```

Design points:

- **`Arc<BinObject<M>>` is the currency.** A hit is an `Arc` clone: callers keep values as
  long as they like, eviction never invalidates anything, and the values cross threads. The
  return type is the part of this that could never have been retrofitted (ADR-0011).
- **The box requires `Send`** (`Box<dyn ObjectCache<M> + Send>`, with `M: Send` where it
  matters), so a handle with a cache installed stays `Send` for the manager's per-document
  workers. `Rc`-based providers are ruled out, deliberately.
- **`NoCache` is a real provider, not an `Option`.** The handle always holds a box; the
  default is `NoCache`, under which `cached_object()` parses on every call. One mechanism,
  no special-cased disabled state.
- **Only `cached_object()` consults it.** The cursors and `object()` never touch the cache;
  a sweep does not evict what a consumer is holding hot, and the uncached paths keep
  returning owned values as drafted.
- Dispatch is one vtable call per lookup - noise next to the parse it saves.

### <a id="s4.5"></a>4.5 Batch lookup

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
pub struct BatchObjects<'a, R: io::Read + io::Seek, M = NoMeta> { /* ... */ }

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
  randomly. Offset order is what the internal buffer and the OS readahead reward - and it
  is also simply file order, which is why both the cold and the warm path can promise the
  same yield order.
- **Yield order is file order, documented.** A caller that needs request order collects
  and reorders - it has the hashes. Promising request order would force the handle back
  into random seeks and cost the whole point.
- **Cold handles finish early.** `object()` completes the full TOC scan before answering.
  A batch knows its request set, so the scan can stop at the last hit - on a request for
  objects near the front of a large bin, most of the table is never read. The rows the
  scan did pass still land in the TOC as always.
- **Misses are data, not yields.** `next` skips absent hashes; `missing()` reports them
  after exhaustion. Yielding a `None`-per-miss in file order would be unanswerable (a
  miss has no file position).
- **One open object at a time**, same lending shape as [`Objects`] and for the same
  borrow reason.

The API lands with the foundation surface but earns its keep once `view()`/`read()`
([section 4.2](#s4.2) and [section 4.3](#s4.3)) exist: descriptors alone are answered by the TOC
without seeking, and it is batch *body* reads where the monotonic schedule pays.

## <a id="s5"></a>5. API surface - `PTCH`

```rust
/// A mounted `PTCH` stream: outer header, delete list, and the inner `PROP` header
/// are parsed; embedded objects and patch records stream on demand.
pub struct BinOverrideStream<R: io::Read + io::Seek, M = NoMeta> { /* ... */ }

impl<R: io::Read + io::Seek, M: Default> BinOverrideStream<R, M> {
    pub fn mount(source: R) -> Result<Self, Error>;

    /// Entry hashes the layer deletes from its base bin - the outer header's
    /// `count x u32` list, mirroring `BinOverride::deleted`.
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
pub struct Patches<'a, R: io::Read + io::Seek, M = NoMeta> { /* ... */ }

impl<'a, R: io::Read + io::Seek, M: Default> Patches<'a, R, M> {
    pub fn next(&mut self) -> Result<Option<PatchStream<'_, R, M>>, Error>;
}

/// One patch record: the addressing half is read, the value is not. A record is
/// self-delimiting via its payload size, so skipping is a seek.
pub struct PatchStream<'a, R: io::Read + io::Seek, M = NoMeta> { /* ... */ }

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

`objects()`, `entries()`, `toc()` and `object()` cover the embedded objects only and never
touch a record: the object table precedes the record list in the file, and a consumer that reads
a patch bin for the content the game loads - the walk of `value-walk.md`
[section 6](value-walk.md#s6) is one - sees its objects and nothing of its records. `patches()`
is the only cursor over the record list (S24).

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

## <a id="s6"></a>6. Skip semantics

Skipping mirrors `MetaValue_skipByType`: primitives by fixed width, strings by length
prefix, complex values (`Container`, `UnorderedContainer`, `Struct`, `Embedded`, `Map`) by
their stored byte size, `Optional` by recursing into its zero-or-one element, `BitBool` as
one byte. Whole objects and whole patch records skip by their own size fields. No skip
allocates or decodes value contents.

At the file level a skip is a seek; inside a buffered object it is slice arithmetic over
the view. Same rules, two costs, one implementation in the layout core ([section 9](#s9)).

## <a id="s7"></a>7. Strictness: counts drive the parse, sizes drive the skips, disagreement is an error

The client trusts counts when parsing and reads sizes only to skip, and never checks that the two
agree. A stream needs both numbers at once, so it cannot leave the question open; where it lands,
and why that is worth diverging from the client for, is ADR-0009.

The two paths and their trust model:

- **Skip path**: the declared size is the seek distance. There is nothing else to trust,
  and a value the parse path would reject still skips cleanly by its size - which is
  also what the client does with it.
- **Parse path**: counts drive the walk over the buffered bytes, exactly as they drive
  the client's parser. A sized region's declared size is compared against what the
  counts consumed after the fact, and a disagreement is `Error::InvalidSize(declared,
  consumed)` - the same variant the eager readers have always raised for this condition.
  One condition, one error, on both paths.

That unification is what keeps `Bin::from_reader`'s behavior unchanged when it is rebuilt
over the stream ([section 9](#s9)): the inline checks in the `ReadProperty` impls and the walk's
check are the same check raising the same error. The homogeneity failures stay where they
are, hard errors from the value model's checked constructors (`InvalidNesting`,
`InvalidKeyType`, `MismatchedContainerTypes`).

A consumer surveying broken or hand-crafted files catches the error per chunk - tooling
built on the error, not state built into the core. After a mismatch the handle's
sequential sweep is not trustworthy (the mismatch is the proof of that); random access
through the already-harvested TOC rows remains valid, since those offsets tiled correctly
up to the failure.

## <a id="s8"></a>8. The legacy-numbering latch

The eager reader detects legacy property-kind numbering by failing on a kind byte and
re-reading the whole object table with the legacy mapping. The stream latches instead:

- The handle starts in current numbering.
- When decoding a kind byte fails (`Error::InvalidPropertyTypePrimitive`) during any parse or skip,
  the current *object* is re-read from its own start in legacy numbering. That is why buffering
  walks an object as it lands rather than lazily ([section 4.2](#s4.2)): the latch has to be settled
  before any view exists, and a view cannot flip it from behind a shared reference. If that
  succeeds, the handle latches legacy for the rest of its life. With buffered objects the retry is a
  re-walk of bytes already in memory - no I/O. A view captures the flag at creation, so a view
  handed out before the latch keeps the numbering it was built under.
- Objects yielded before the latch are not revisited; a streaming consumer that already
  acted on them acted on data parsed under the wrong mapping only if those objects happened
  to parse cleanly both ways - the same ambiguity the eager retry has, narrowed to a prefix.
- `into_bin()` removes the asymmetry: on latch it restarts the drain from the top of the
  object table, reproducing the eager reader's behavior exactly.

As today, the retry can reinterpret a genuinely desynced file as "legacy"; the latch does
not widen that hazard, and a latched handle reports it (`fn numbering(&self) -> Numbering`).

Mechanically the latch is nothing new: `Kind::unpack(raw, legacy)` already centralizes the legacy
fudging for every kind byte in the crate. What the stream adds is where the flag comes from - a
[`Numbering`] carried by the cursor over the bytes rather than an argument threaded through each
operation ([section 9](#s9)). `BinStream::numbering()` and `ObjectView::numbering()` are what the
latch reports, with `Numbering::is_legacy()` one call away when a boolean is what a caller wants.

## <a id="s9"></a>9. Layering: one layout core under two renderers

There are two ways to decode a value - borrowed out of bytes, and owned into
`PropertyValueEnum` - so the single-decode-path rule sits one level below the entry points:
**one module owns the layout**, and both surfaces are renderers over it (ADR-0008).

```text
       layout core (offsets, shape, skip, leaf codecs over &[u8])
         /                         \
  ReadProperty impls          ObjectView / ValueView
  (owned PropertyValueEnum)   (borrowed, zero-copy)
         \                         /
     into_bin() == from_reader   corpus parity sweep
```

`Bin::from_reader` is:

```rust
pub fn from_reader<R: io::Read + io::Seek + ?Sized>(reader: &mut R) -> Result<Self, Error> {
    BinStream::mount(&mut *reader)?.into_bin()
}
```

and `BinOverride::from_reader` likewise drains a `BinOverrideStream`. The top-level loops exist
once, in the stream.

**The module is `stream::layout`, and it is crate-internal.** It owns where a value starts, how far
it runs, what its header declares, and the leaf codecs over `&[u8]`. Publishing a thirty-method
cursor would pin it under semver for nobody's benefit, so only [`Numbering`] is re-exported - it is
what the latch reports.

**The numbering is cursor state, and every walk is a method.** Each layout operation has the cursor
as its subject - `cur.skip_value(kind)`, `cur.walk_value(kind)`, `cur.walk_object()`,
`cur.value_shape(kind)`, `cur.sized_region(..)`, `cur.take_value(kind)` - and the numbering never
varies within one cursor's life, because it is the context the bytes were written in rather than an
argument to each operation. So a `Cursor` carries a [`Numbering`], and a slice and its numbering
travel together where they cannot be paired up wrongly. `take_value(kind)` hands out a value's
bytes in one call, which is what keeps the views from doing a note-the-position, skip, slice-back
dance.

`Kind::fixed_width()` lives on `Kind` rather than in the module: it is a fact about a kind rather
than about a position, which is the same sort of fact as `is_primitive`, `subtype_count` and
`is_valid_map_key`. That leaves the layout module as exactly one type and one enum.

**Size checking happens in the walk, once.** The `ReadProperty` impls used to verify sizes inline -
`Container::from_reader` measuring its own body and raising `Error::InvalidSize`. That check is the
layout core's walk now, raising the same error from one place ([section 7](#s7)). The homogeneity
checks - `InvalidNesting`, `InvalidKeyType`, `MismatchedContainerTypes` - stay in the value model,
because they are model invariants rather than stream policy, and the views surface them from the
same core.

**One decode path needs a reader bridge.** `ReadProperty`'s signature is public and its breaking
window is closed, so it still takes an `io::Read + io::Seek`. Only the layout core knows how far a
value reaches, so the impls for the self-sized kinds gather their bytes by growing a buffer until
`walk_value` can cross it, then wind the reader back over the over-read - which keeps the extent
rules in one place rather than inventing a second set. Driving that probe with the **walk** rather
than the skip is what preserves `Error::InvalidSize`: a declared size the counts disagree with
still raises, instead of turning into an early EOF. `BinObject::from_reader` needs no probe, since
the object's own size field bounds it.

The fixed-width primitives keep their direct reader codecs. Routing them through the bridge would
tighten their bounds - they read from a bare `io::Read` today - and allocate per leaf, for no
behavioural gain, and nothing on the parse path reaches them any more. A unit test pins the two
codec families to each other so they cannot drift unnoticed.

**Two behaviour changes came out of the unification**, both the byte-level codec's answer replacing
the reader's: a string that is not UTF-8 raises `Error::Utf8Error` rather than `Error::ReaderError`,
and `Bin::from_reader` buffers internally, so it no longer leaves the reader at a defined position.
No caller in the workspace read from it afterwards.

**Two divergences the renderers keep, deliberately.** `ObjectView::property` returns the first
property with a name hash while the owned side's keyed map keeps the last, and
`ObjectView::property_count` is the wire count while `BinObject::properties` is deduplicated. Both
differ only for an object declaring one name hash twice, which no shipped bin does, and closing
either would cost every lookup the early exit it has. Documented on the methods rather than paid
for.

## <a id="s10"></a>10. The write-back contract: delta rewrite of a `.bin`

Added for the bin editor's flow (expand -> lazily read one object -> edit -> save). The save
target is a **rewritten `.bin` file**. Authoring the edit as a `PTCH` layer is explicitly
out of scope: that layer is not yet designed for mods, and nothing here forecloses it -
a delta is upstream of either output form.

This section fixes the *contract*; the implementation is a later stage ([section 11](#s11)).

### <a id="s10.1"></a>10.1 The editor flow over this API

1. **Expand / navigate**: `mount()` + `toc()` give the object rows (path, class, size)
   without parsing; browsing an object read-only is `object(hash)` -> `view()`.
2. **Read for editing**: `object(hash)?.read()` -> an owned `BinObject<M>`. The editing
   path takes `read()`, never `cached_object()` - the cache hands out shared `Arc`s, and
   an edit wants exclusive ownership. (Cache for viewing, `read()` for editing;
   `Arc::make_mut` is the escape hatch when both are wanted.)
3. **Edit**: the crate's mutation surface - `resolve_mut(&PropertyPath)` -> `ValueSlot` ->
   `ValueMut`, or structured operations that are shape-for-shape the editor's patch table.
   The edited object goes into the document's delta; undo is inverse patches over it.
4. **Save**: `write_patched` to a temp file, rename over. After a rename-over, the mounted
   handle still describes the *old* bytes - the consumer remounts.

### <a id="s10.2"></a>10.2 The types

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

### <a id="s10.3"></a>10.3 Invariants

- **Untouched means bit-identical.** An object the delta does not name is never
  deserialized - its bytes are copied from `byte_range()`. This is a stronger guarantee
  than the editor's current "the backend owns the tree" model: a kind with no widget, a
  hash no table names, a container order, a duplicate key - none of it can be lost,
  because none of it is interpreted.
- **The version passes through.** The header writes the version that was read, so saving
  one edit does not upgrade the file - the version-3-rewrite hazard the editor documents
  applies only to objects that were actually edited (which re-encode through the current
  writer), not to the file.
- **A legacy-latched base refuses the delta write.** Raw-copied objects would keep the
  legacy kind numbering while re-encoded ones wrote modern numbering - a mixed, corrupt
  file. A legacy-latched handle gets a dedicated error; the consumer falls back to a full
  `into_bin()` + `to_writer` transcode, or opens read-only. Shipped files are modern, so
  this is a guard, not a path.
- **Size mismatches cannot reach this path.** Raw copy-through never walks an unedited
  object, so a lying size field in one is copied exactly as its declared range states,
  reproducing the input byte for byte. An *edited* object was necessarily read, and a
  size mismatch there already failed the read with `Error::InvalidSize` ([section 7](#s7)).

### <a id="s10.4"></a>10.4 What this deliberately is not

- **Not `PTCH` authoring.** A `BinDelta` could later *also* render as patch records -
  the operations are shape-compatible - but that output form waits until the patch layer
  is designed for mods.
- **Not a mutable view.** No `ValueViewMut` over the buffered bytes: in-place byte
  mutation only works for fixed-width leaves (a string edit shifts every size field above
  it), so owned-`BinObject`-per-edited-object is the right granularity, and
  `ValueSlot` already guards mutation on the owned side.
- **Not an in-place file update.** The write always produces a complete new stream
  (temp + rename at the consumer's discretion); offsets shift freely and nothing is
  patched into the middle of a file.

## <a id="s11"></a>11. What this deliberately does not do (yet)

- **Streaming `resolve`.** `ValueView` already delivers lazy descent ([section 4.3](#s4.3)). What is
  not here is `ObjectStream::resolve(&PropertyPath)`, the loop that walks a path over the views with
  the resolver's traversal and type rules (PRD-001). It stays a named follow-on: it is thin, and
  thin is exactly when a feature should wait for a consumer.
- **Writing.** The stream is read-only. The write-back *contract* - a delta rewrite of a whole
  `.bin` by copy-through of untouched objects' raw bytes plus re-serialization of edited ones - is
  specified in [section 10](#s10), because the bin editor's flow depends on its shape; the
  implementation is a later stage.
- **Parallel access.** One cursor at a time per handle, `&mut self` throughout. The
  fan-out workloads parallelize per file, not within one.
- **Caching by default.** `object()` and the cursors parse on every call and return owned data.
  Caching exists only as the opt-in `cached_object()` path through the installed [`ObjectCache`]
  provider ([section 4.4](#s4.4)). A consumer that wants a resident tree uses `into_bin()`.

## <a id="s12"></a>12. Testing

`corpus.rs`, ignored unless `LTK_LOL_GAME_DIR` points at an install, is where stream parity lives:
the harness already sweeps every archive in a live install, so the stream adds its checks there
rather than growing a second one. For every `PROP` and `PTCH` chunk:

- `entries()` harvests the same `(path_hash, class_hash)` set the eager parse holds;
- `into_bin()` equals `Bin::from_reader`;
- `object(hash)` on a sample equals the eager lookup, and a batch of the same hashes opens the same
  objects;
- every property is viewed, shaped and decoded against the eager parse;
- every declared size equals `PropertyExt::size` over the parsed values, which attests that shipped
  files are size-clean and not merely parse-clean, and is the debug-assert cross-check for skip
  distances.

What that sweep currently attests is [appendix A](#appendix-a).

Beyond the corpus: the two leaf codec families - the layout core's `&[u8]` codecs and the
fixed-width primitives' direct reader codecs - are pinned to each other by a unit test so they
cannot drift unnoticed ([section 9](#s9)), and a file written in legacy numbering reads identically
through the stream and the eager path.

## <a id="s13"></a>13. Rules

Every rule too small to hold a section of its own, in one table, ordered by subject. **Rule** is
what the crate does, **Instead of** the alternative weighed and rejected, **Spec** where the
behaviour is specified in full - so nothing is restated here. A row whose Spec names an **ADR** is
argued there, with the options it beat and what it costs; the row states the rule and no more.

`Sn` is a stable citation key. A rule that changes keeps its ID and has its row rewritten; new
rules append.

| ID | Rule | Instead of | Why | Spec |
| -- | ---- | ---------- | --- | ---- |
| S1 | The handle owns its source and buffers internally (`BufReader` + `seek_relative`); callers hand over the bare `File` and `into_inner()` unwraps. | Borrowing the source, or taking a caller-supplied `BufReader`. | A caller-supplied `BufReader` is worse than nothing - plain `seek` discards its buffer - and the type system cannot catch that, so the handle owns the problem. Owning also matches `ltk_wad::Wad`. | [section 4](#s4) |
| S2 | Mounting reads the header, dependencies and class table and stops. The offset index is built on first random access; iteration streams without one. | Indexing the object table at mount. | Harvesting and random access want different work, and neither should pay the other's. A fully-swept handle has the index for free, because the sweep populates it. | [section 4](#s4) |
| S3 | An object's bytes are buffered once and viewed zero-copy; `ValueView` descends to any depth without materializing. | Forward-only lending cursors over the reader, at property granularity. | An object's size is known before descending and objects are KB-scale, so buffering bounds memory as tightly while dissolving every restriction the cursors imposed. | [section 4.2](#s4.2), [section 4.3](#s4.3); ADR-0007 |
| S4 | A lending cursor at the file level, a `std` iterator of descriptors over it, views inside an object. | One uniform iterator shape throughout. | A yielded object borrows the reader, which is what enforces one open object at a time, so the file level cannot be a `std` iterator. Nothing inside a buffered object borrows the reader, so everything there can be. | [section 4.1](#s4.1), [section 4.3](#s4.3) |
| S5 | `objects()` and `entries()` always restart from the top of the table. | Resumable cursors that remember a position. | Idempotent and free of hidden state; resumption is what the TOC and `object(hash)` exist for. | [section 4](#s4) |
| S6 | `BinToc` is plain data: `Clone`, serializable behind the `serde` feature, detachable from the handle. | An index private to the handle. | The manager's object index wants to persist it and iterate it without holding the file open. | [section 4](#s4) |
| S7 | `objects_batch` schedules by file offset, yields in file order, and reports misses through `missing()`. | Yielding in request order, or a `None` per miss. | Hash order has no relationship to file position, so a request-ordered batch still seeks randomly - which is the whole cost it exists to remove. A miss has no file position, so it cannot be yielded in file order at all. | [section 4.5](#s4.5) |
| S8 | One module owns the layout - offsets, shapes, skip distances and the leaf codecs over `&[u8]` - and the owned impls and the views are renderers over it. | A separate streaming parser beside the eager one. | Two decoders over one wire format drift, and the drift is silent until the first file they disagree about. | [section 9](#s9); ADR-0008 |
| S9 | `Bin::from_reader` is `mount` plus `into_bin`. | Keeping the eager reader as its own parser. | Makes the stream the crate's only parser, so a fix to one is a fix to both. | [section 9](#s9); ADR-0008 |
| S10 | The layout module is crate-internal; only `Numbering` is re-exported. | Publishing the cursor. | Nothing outside the crate uses it, and publishing a thirty-method cursor pins it under semver for nobody's benefit. | [section 9](#s9) |
| S11 | Counts drive the parse, declared sizes drive the skips, and a size the walk disagrees with is `Error::InvalidSize`. | A tolerant discrepancy log, as the client is tolerant. | Continuing past the mismatch means handing out TOC rows and byte ranges built from sizes the parse just disproved. | [section 7](#s7); ADR-0009 |
| S12 | `M` lives on the handle, and a `concrete` alias pins it at the `mount` call. | `M` on the value-producing methods. | Handle placement is the only one an alias can reach, and the alias is what a consumer depends on, because `mount` is itself expression position. | [section 4](#s4); ADR-0010 |
| S13 | The public error enums are `#[non_exhaustive]`, taken in the 0.8.0 breaking window. | Adding variants as breaking changes later. | The streaming work grows variants for years; the free breaking moment was the one to spend. | `ptch-property-patches.md` [section 6](ptch-property-patches.md#s6) |
| S14 | Caching is an opt-in provider on the handle, `NoCache` by default, returning `Arc<BinObject<M>>`. Only `cached_object()` consults it. | An `Option<Cache>` returning a borrow, or a policy enum and a third type parameter. | Eviction must never invalidate a value a caller holds, and a sweep must not evict what a consumer is holding hot. | [section 4.4](#s4.4); ADR-0011 |
| S15 | The latch re-reads the current object under legacy numbering, then latches for the handle's life; `into_bin` restarts from the top. | Re-reading the whole file, as the eager reader does, on every discovery. | With buffered objects the retry is a re-walk of bytes already in memory. Restarting `into_bin` is what keeps the eager path's behaviour identical. | [section 8](#s8) |
| S16 | `Kind::unpack(raw, legacy)` is the latch mechanism, fed from a `Numbering` the cursor carries. | A `legacy: bool` threaded through every layout operation. | The fudging is already centralized, and the numbering is the context the bytes were written in rather than a per-call argument - so a slice and its numbering travel together and cannot be paired up wrongly. | [section 8](#s8), [section 9](#s9) |
| S17 | `ValueShape` is the streaming peek: `PropertyView::shape()` returns the resolver's own descriptor. | A shape type private to the stream. | A complex value's wire header carries exactly what the type rule compares, so one vocabulary spans the resolver, the patch checker and the stream at skip cost. | [section 4.3](#s4.3) |
| S18 | `read()`, not `parse()`. | `parse()`. | It does I/O, and the crate's vocabulary is `from_reader` / `ReadProperty`; `parse` appears nowhere in the API. | [section 4.2](#s4.2) |
| S19 | The view iterators are named types (`Properties`, `ContainerItems`, `MapEntries`). | `impl Iterator` returns. | A returned `impl Iterator` cannot be named by a caller storing one, and would have had to spell out its lifetime capture anyway. The named types also carry `Debug`, `Clone`, `FusedIterator` and an exact `size_hint`. | [section 4.3](#s4.3) |
| S20 | Every view is `Copy` for every `M`, with the impls written by hand. | Deriving `Clone`, `Copy` and `Debug`. | `M` is a phantom, so a derive would demand `M: Copy` for a field that holds nothing. | [section 4.3](#s4.3) |
| S21 | Scope is `PROP` and `PTCH`, reading only, with `into_bin()` as the upgrade. | Including the write path in the same stage. | The delta pipeline is a later stage this design must not preclude, and [section 10](#s10) is what keeps that promise checkable. | [section 10](#s10), [section 11](#s11) |
| S22 | A save is a delta rewrite of the whole `.bin`: untouched objects copied through byte-exactly, edited ones re-encoded. PTCH authoring is out of scope. | Patching bytes in place, or emitting a `PTCH` as the save format. | A delta is upstream of either output form, and nothing here forecloses rendering one as patch records later. | [section 10](#s10) |
| S23 | `BinToc::largest` answers the largest declared object size before any body is decoded. | Leaving the consumer to fold over `entries()`. | It is the number a consumer bounds a streamed read by, and naming it says that the TOC is where it comes from. | [section 4](#s4) |
| S24 | A `PTCH` stream's object cursors yield embedded objects only; `patches()` alone reads records. | One cursor interleaving objects and records. | The two are different content: objects are what the game loads, records are edits to a base it does not hold. A consumer walking content wants the first and never the second. | [section 5](#s5) |
| S25 | `ObjectView`, `StructView` and `ValueView` implement the walk's `TreeNode` and `TreeValue`; `BinStream::walk` sweeps a file through a visitor with nothing materialised. | A walk over the owned tree with `read()` per object. | The views exist so a consumer pays for what it reads; a pass that decoded every object to visit it would pay for everything. | `value-walk.md` [section 3](value-walk.md#s3), [section 5](value-walk.md#s5); ADR-0014 |

## <a id="appendix-a"></a>Appendix A. Corpus measurements

The parity sweep of [section 12](#s12), run over a live install:

| measurement | result |
| --- | --- |
| archives swept | 392 |
| `PROP` chunks | 48,912 |
| objects | 454,073 |
| properties viewed, shaped and decoded against the eager parse | 2,472,864 (all of them) |
| chunks where `into_bin()` differed from `Bin::from_reader` | 0 |
| chunks where a declared size differed from `PropertyExt::size` | 0 |
| chunks latching onto the legacy numbering | 0 |

Every property is checked rather than a sample, and a batch of sampled hashes per chunk opens the
same objects the per-hash lookups do.

The last row is the interesting one: nothing in a shipped install is written in the legacy
numbering, which is what [section 8](#s8) expects. The latch is a guard, not a path - it exists for
files older than anything Riot currently ships, and it is untested against real data by definition.

## <a id="appendix-b"></a>Appendix B. Cache measurements, 2026-09-01

What [section 4.4](#s4.4) is worth, measured rather than assumed. Run against a live install
(40 of its 392 archives, 2,740 `PROP` chunks), with the sampled chunks written to disk and
mounted as files, so a miss pays the seek and the read a real consumer pays. Best of five runs.

**What a hit saves.** The 150 heaviest link sequences, 41.7 MB:

| | per object |
| --- | --- |
| miss - seek, read, parse | 30,010 ns |
| hit - an `Arc` clone | 105 ns |
| saved per hit | 29,905 ns (286x) |

**Pattern 1, harvested: an editor chasing `ObjectLink`s.** Every in-file link target in
traversal order, which is the only cache-relevant access pattern a shipped file can attest to:

| measurement | result |
| --- | --- |
| files carrying in-file links | 1,833 of 2,740 |
| link references followed | 20,412 |
| distinct targets | 17,321 |
| ceiling hit rate, unbounded cache | 15.1% |
| files where any target is referenced twice | 385 (21%) |
| most references to one target | 499 |
| `NoCache` to `LruObjectCache(32)` | 1.14x |
| `NoCache` to `LruObjectCache(128)` | 1.20x |

**Pattern 2, modelled: a working set re-requested.** The manager's "same objects across
requests" has no signature in a file, so it is modelled rather than harvested - 16 objects per
file, requested 8 times each. Labelled as a model on purpose:

| policy | vs `NoCache` |
| --- | --- |
| `LruObjectCache(16)` | 8.29x |

**What ties them.** From the two costs above, speedup is approximately `1 / (1 - hit_rate)`.
That predicts 1.18x at pattern 1's 15.1% and 7.81x at pattern 2's 87.5%, against 1.14x-1.20x
and 8.29x measured - so a consumer can predict its own payoff from its own hit rate without
re-running any of this.

Two things the numbers do not say. The miss cost is warm: the file is in the page cache by the
second run, so 286x is a floor rather than a ceiling. And pattern 1 is the weak one - the
corpus attests 1.2x, not 8x. The case for the cache is that it costs a consumer who does not
want it nothing, because `NoCache` is the default and only `cached_object` consults it
([section 4.4](#s4.4), rule S14), while paying 8x for the interactive consumers ADR-0011 names.
