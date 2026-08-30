---
issue: 208
title: "Bin streaming: zero-copy object views"
labels: crate:ltk_meta, enhancement, format:bin, area:reading, blocked
---

Part of #192 (design: `docs/design/bin-streaming.md` sections 4.2-4.3, section 8, section 14). Descending into an object buffers its declared byte range once (one read, handle-owned reused buffer) and everything inside — iteration, random access, descent — happens in memory, zero-copy. Nothing decodes until touched, nothing allocates until an owned value is asked for.

## Proposed surface

```rust
impl<'a, R: io::Read + io::Seek, M: Default> ObjectStream<'a, R, M> {
    /// Buffers the object's declared byte range and returns a zero-copy view over it.
    pub fn view(&mut self) -> Result<ObjectView<'_, M>, Error>;
}

/// One object's bytes, viewed in place.
pub struct ObjectView<'a, M = NoMeta> { /* path/class hashes, a cursor over the bytes */ }

impl<'a, M> ObjectView<'a, M> {
    pub fn path_hash(&self) -> BinHash;
    pub fn class_hash(&self) -> BinHash;
    /// What the file declares, which is what `properties()` yields.
    pub fn property_count(&self) -> u16;
    /// Which property-kind numbering the object was read under.
    pub fn numbering(&self) -> Numbering;

    /// The properties in file order. A real `std` iterator; items are `Result` because
    /// a header's kind byte can fail to decode.
    pub fn properties(&self) -> Properties<'a, M>;

    /// Random access by name hash — an in-memory walk, no index needed. The first
    /// property with that hash; the owned side's map keeps the last, which only differs
    /// for an object that declares one hash twice.
    pub fn property(&self, name_hash: impl Into<BinHash>)
        -> Result<Option<PropertyView<'a, M>>, Error>;

    /// The object's raw bytes, for the rebase pipeline's copy-through.
    pub fn raw(&self) -> &'a [u8];
}

/// One property: header decoded, value untouched.
pub struct PropertyView<'a, M = NoMeta> { /* name hash, kind, a cursor over the value */ }

impl<'a, M> PropertyView<'a, M> {
    pub fn name_hash(&self) -> BinHash;
    pub fn kind(&self) -> PropertyKind;

    /// The shape the value declares — a container's item kind, a map's key and value
    /// kinds, an embed's class — from the few header bytes ahead of the body. Same
    /// [`ValueShape`] the resolver's type rule uses (#187).
    pub fn shape(&self) -> Result<ValueShape, Error>;

    /// For containers and maps, the element count from the same header bytes. `None`
    /// for every other kind, an option included: `OptionalView::is_some` answers for one.
    pub fn item_count(&self) -> Result<Option<u32>, Error>;

    /// The value's raw bytes (header excluded).
    pub fn raw(&self) -> &'a [u8];

    /// Descends into the value without materializing it.
    pub fn value_view(&self) -> Result<ValueView<'a, M>, Error>;

    /// Decodes the value — the whole subtree — into the existing owned representation.
    /// (`M: Default`.)
    pub fn value(&self) -> Result<PropertyValueEnum<M>, Error>;
}
```

`ValueView` is the borrowed mirror of `PropertyValueEnum`, one variant per `Kind`:

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

impl<'a, M> ContainerView<'a, M> {
    pub fn item_kind(&self) -> PropertyKind;
    pub fn len(&self) -> u32;
    pub fn is_empty(&self) -> bool;
    pub fn iter(&self) -> ContainerItems<'a, M>;
    /// One offset calculation for a fixed-width item kind; a walk otherwise.
    pub fn get(&self, index: u32) -> Result<Option<ValueView<'a, M>>, Error>;
}

impl<'a, M> StructView<'a, M> {
    /// `0` for a null pointer, which has no size field and no body.
    pub fn class_hash(&self) -> BinHash;
    pub fn property_count(&self) -> u16;
    pub fn properties(&self) -> Properties<'a, M>;
    pub fn property(&self, name_hash: impl Into<BinHash>)
        -> Result<Option<PropertyView<'a, M>>, Error>;
}

// MapView: key_kind() / value_kind() / len() / is_empty() / iter() of (key, value) pairs.
// OptionalView: item_kind() / is_some() / is_none() / get() -> Result<Option<ValueView>, Error>.
// Properties / ContainerItems / MapEntries are named `std` iterators, so a caller can
// store one; every view is `Copy` whatever `M` is, because `M` is a phantom here.
```

Because `ValueView` descends to any depth, `Elements[3].Position` is a `ContainerView::get` and two `StructView::property` calls, none of which materialize a sibling. (The rationale for views over forward-only cursors: `PropertyValueEnum` is 96 bytes per node, align 16 — a wire `f32` costs 96 bytes materialized. Buffering one KB-scale object dissolves the cursor restrictions while the file-level sweep keeps the constant-memory guarantee. section 14.)

## The numbering latch

Lands at this layer (section 8): a kind byte that fails to decode re-walks the already-buffered object under the legacy numbering (no I/O), latches the handle for the rest of its life, and reports via `numbering()`. A view captures the numbering at creation, so one handed out before the flip keeps what it was built under.

The numbering is cursor state rather than a flag threaded through every call: `stream::layout::Cursor` carries the `Numbering` its bytes were written under, and `Kind::unpack` is fed from that. A slice and its numbering travel together and cannot be paired up wrongly.

Demoable: a corpus example greps field-level — every property of a chosen shape (e.g. `String` equal to a target, or `Container[ObjectLink]`) across an install — with zero owned values built.

- [ ] `view()` reads the object's bytes exactly once; iteration, `property(hash)`, and nested descent are all in-memory
- [ ] `ValueView` covers every kind; strings validate UTF-8 on access; sub-views descend to arbitrary depth
- [ ] `shape()` agrees with `ValueShape::of` on the parsed value for every corpus object sampled
- [ ] Fixed-width container items index by arithmetic; others by walk
- [ ] A synthetic legacy-numbered file latches, re-walks, parses; `numbering()` reports it
- [ ] Size-lying synthetic input fails the walk with `Error::InvalidSize`
- [ ] `cargo fmt`, `clippy --all-targets`, `doc --no-deps` clean
