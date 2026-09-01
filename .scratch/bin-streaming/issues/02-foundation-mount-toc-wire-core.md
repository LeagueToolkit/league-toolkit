---
issue: 207
title: "Bin streaming foundation: mount, TOC, harvest, and the wire core"
labels: crate:ltk_meta, enhancement, format:bin, area:reading, blocked
---

First slice of #192 (design: `docs/design/bin-streaming.md` [section 4](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/bin-streaming.md#s4), [section 6](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/bin-streaming.md#s6)-[section 7](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/bin-streaming.md#s7), [section 9](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/bin-streaming.md#s9)). A consumer can mount a real `.bin` and read everything that costs no value parsing. Everything lives in `ltk_meta::stream`, re-exported from the crate root; `M` sits on the handle and `concrete` grows stream aliases that pin it at the mount call.

## Proposed surface

```rust
/// A mounted `PROP` stream: the header is parsed, the object table is read on demand.
///
/// Owns its source and buffers internally (`BufReader` + `seek_relative`). Hand it the
/// bare `File`; pre-wrapping in `BufReader` only double-buffers.
pub struct BinStream<R: io::Read + io::Seek, M = NoMeta> {
    /* buffered reader, header, lazy toc, latch, cache */
}

impl<R: io::Read + io::Seek, M: Default> BinStream<R, M> {
    /// Mounts a `PROP` stream, reading the header, dependencies and class-hash table.
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
    /// objects not descended into are skipped by their size field.
    pub fn objects(&mut self) -> Objects<'_, R, M>;

    /// A `std` iterator of plain descriptors, for harvesting and filtering.
    pub fn entries(&mut self) -> Entries<'_, R, M>;

    // -- random access -------------------------------------------------------
    /// The table of contents: every object's `(path_hash, class_hash, offset, size)`.
    /// Built by one harvest sweep on first use, then cached. Sweeps also populate it
    /// as a side effect, so a fully-swept handle pays nothing.
    pub fn toc(&mut self) -> Result<&BinToc, Error>;

    /// Opens the object with the given path hash, building the TOC if needed.
    pub fn object(&mut self, path_hash: impl Into<BinHash>)
        -> Result<Option<ObjectStream<'_, R, M>>, Error>;

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

/// File-order entries plus a hash index. Plain data: `Clone`, serializable behind the
/// `serde` feature, so a consumer can detach and persist it.
#[derive(Debug, Clone)]
pub struct BinToc { /* Vec<ObjectEntry> + HashMap<BinHash, usize> */ }

impl BinToc {
    pub fn entries(&self) -> &[ObjectEntry];
    pub fn entry(&self, path_hash: impl Into<BinHash>) -> Option<&ObjectEntry>;
}
```

```rust
/// Streaming cursor over the object table. Not a `std` iterator: each yielded
/// [`ObjectStream`] borrows the reader, so the borrow checker enforces one open
/// object at a time.
#[must_use = "cursors are lazy and read nothing until advanced"]
pub struct Objects<'a, R: io::Read + io::Seek, M = NoMeta> { /* … */ }

impl<'a, R: io::Read + io::Seek, M: Default> Objects<'a, R, M> {
    /// Advances to the next object, skipping whatever the previous one did not consume.
    pub fn next(&mut self) -> Result<Option<ObjectStream<'_, R, M>>, Error>;
}

/// `std` iterator of plain [`ObjectEntry`] descriptors — `Objects` without descent.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct Entries<'a, R: io::Read + io::Seek, M = NoMeta> { /* Objects<'a, R, M> */ }

impl<R: io::Read + io::Seek, M: Default> Iterator for Entries<'_, R, M> {
    type Item = Result<ObjectEntry, Error>;
}
```

`ObjectStream` lands here with its descriptor surface (`path_hash`, `class_hash`, `entry`, `byte_range`, `property_count`); `view()` and `read()` are the follow-up issues.

## Beneath it: the wire core

One byte-level module owns the wire (expand phase — existing readers untouched): value shapes (`ValueShape` from the wire header), skip distances for every kind, leaf codecs over `&[u8]`, and legacy numbering threading via `Kind::unpack`. Skip semantics mirror `MetaValue_skipByType`: primitives by fixed width, strings by length prefix, complex values by their stored byte size. The parse path is driven by counts; a declared size that disagrees with what the counts consumed is `Error::InvalidSize`, the same error the eager readers raise for it ([section 7](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/bin-streaming.md#s7)).

Demoable: a corpus test harvests `(path_hash, class_hash)` for every PROP chunk in an install and matches the eager parse — the grep-index workload end to end.

Blocked by #206 (#187 must merge first).

- [ ] Mount on a shipped `.bin` exposes version, dependencies, class hashes with no seeking past the header
- [ ] `entries()` / `toc()` harvest matches the eager parse's object set across an install (corpus test, gated on `LTK_LOL_GAME_DIR`)
- [ ] A repeated sweep reuses the TOC (no second harvest pass)
- [ ] Wire-core skip distances cross-check against `PropertyExt::size` on parsed corpus values
- [ ] Mounting a PTCH file fails with the kind mismatch error; unknown magic fails with the signature error
- [ ] `cargo fmt`, `clippy --all-targets`, `doc --no-deps` clean
