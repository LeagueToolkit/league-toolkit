---
issue: 211
title: "Bin streaming: delta write-back (the editor's save path)"
labels: crate:ltk_meta, enhancement, format:bin, area:writing, blocked
---

Part of #192 (design: `docs/design/bin-streaming.md` section 15). Saving an edit as a rewritten `.bin`. PTCH authoring is explicitly out of scope — a delta is upstream of either output form, and nothing here forecloses rendering one as patch records later.

## Proposed surface

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

## Invariants

- **Untouched means bit-identical.** An object the delta does not name is never deserialized — its bytes are copied from `byte_range()`. A kind with no widget, a hash no table names, a container order, a duplicate key — none of it can be lost, because none of it is interpreted.
- **The version passes through.** The header writes the version that was read, so saving one edit does not upgrade the file; only edited objects re-encode through the current writer.
- **A legacy-latched base refuses the delta write** with a dedicated error — raw-copied objects would keep legacy kind numbering while re-encoded ones wrote modern numbering, a mixed, corrupt file. The documented fallback is a full `into_bin()` + `to_writer` transcode, or read-only.
- **Size mismatches cannot reach this path.** Raw copy-through never walks an unedited object, so a lying size field in one is copied exactly as its declared range states, reproducing the input byte for byte; an edited object was necessarily read, and a mismatch there already failed the read with `Error::InvalidSize`.
- **Not an in-place file update.** The write always produces a complete new stream; the consumer saves to a temp file and renames over, then remounts (the mounted handle still describes the old bytes).

This completes the bin editor's loop end to end: mount → TOC rows → `view()` to browse → `read()` on first edit → mutate through the value-slot surface → `write_patched` to a temp file → rename → remount. (The editing path takes `read()`, never `cached_object()` — the cache hands out shared `Arc`s, and an edit wants exclusive ownership; `Arc::make_mut` is the escape hatch when both are wanted.)

Blocked by #209 (owned decode + eager writer; TOC ranges from #207).

- [ ] An empty delta reproduces the input byte-for-byte, for every PROP chunk in an install (corpus test)
- [ ] A one-property edit re-reads equal to the same edit applied to the eager tree, and every other object's bytes are unchanged
- [ ] A version-1/2 base saves with its header version intact; editing an object re-encodes that object without upgrading the header
- [ ] Removing and appending objects updates the class table and counts consistently (round-trip)
- [ ] A legacy-latched base returns the dedicated refusal error; the error message names the fallback
