---
issue: 210
title: "Bin streaming: PTCH stream"
labels: crate:ltk_meta, enhancement, format:bin, area:reading
---

Part of #192 (design: `docs/design/bin-streaming.md` [section 5](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/bin-streaming.md#s5)). The same treatment for patch bins, including reading the outer header's delete list correctly (`count × u32` entry hashes — fixing the eager reader's historical mis-skip).

## Proposed surface

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

`objects()`, `entries()`, `toc()` and `object()` cover the embedded objects only and never touch a record (S24): the object table precedes the record list, so a consumer sweeping a patch bin for the content the game loads - `ltk-manager`'s problems pass walks every object of every bin, `PTCH` included, and never looks at a record - reads its objects and nothing of its records. `patches()` is the only cursor over the record list. `BinToc::largest()` answers the largest embedded object's size before any is decoded, exactly as for a `PROP` (S23).

`PatchStream::path()` returns `&PropertyPath`, not `&str` — `PropertyPatch::path` is already a `PropertyPath`, so anything less would force callers to re-parse. `BinOverride::from_reader` joins the single decode path (mount + `into_bin_override`).

Parked until a consumer asks, and one is now in sight: `ltk-manager`'s problems pass streams every `PROP` through `BinStream` and falls back to the eager `BinOverride` for a `PTCH`, because `BinStream::mount` refuses one. This ticket removes that fallback. The eager `BinOverride` - with the `data_override/{read,write}.rs` split under it - closed the earlier downstream need, an unimplemented override *write* path, so nothing is blocked on this; it is the one place the manager's streaming path still pays a whole-file parse. Until it lands `BinOverrideStream` and `BinFileStream` do not exist, and the `concrete` aliases [section 4](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/bin-streaming.md#s4) says they want are part of this ticket rather than of the shipped surface.

Demoable: every shipped PTCH file in an install streams — headers, objects, all records — and drains to an eager value equal to the direct eager parse.

- [ ] All shipped PTCH chunks in an install (238 at last count) mount, stream every record, and `into_bin_override()` equals the eager parse
- [ ] A record's object hash, kind and path are readable with the value left untouched; skipping advances by payload size
- [ ] The delete list round-trips (corpus already attests byte-exact outer headers)
- [ ] For every PTCH chunk in an install, `objects()` yields the same objects the eager `BinOverride::objects` holds and reads no byte of the record list (AC-8)
- [ ] `BinFileStream::mount` returns the right variant for PROP, PTCH, and errors on neither
- [ ] Existing PTCH snapshot/round-trip tests pass with `from_reader` on the stream
