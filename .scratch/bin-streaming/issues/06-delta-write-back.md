---
issue: 211
title: "Bin streaming: delta write-back (the editor's save path)"
labels: crate:ltk_meta, enhancement, format:bin, area:writing
---

Part of #192 (design: `docs/design/bin-streaming.md` [section 10](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/bin-streaming.md#s10); requirement PRD-002 FR-12; decision ADR-0016). Saving an edit as a rewritten `.bin`: the mounted base with whole-object edits applied, every untouched object copied byte for byte and only the edited ones encoded. PTCH authoring is out of scope.

The consumers are the bin editor's save and `ltk-manager`'s repair. Both edit a few objects of a
file. The repair reads the objects its findings name with `objects_batch` and `read()`, edits each
with `walk_mut` (value-walk mutable walk), and writes the file with `write_patched` in place of
`into_bin()` + `to_writer`.

## Proposed surface

```rust
/// Whole-object edits held against a mounted base. Costs O(edited objects), not O(file).
#[derive(Debug, Clone, PartialEq)]
pub struct BinDelta<M = NoMeta> { /* ... */ }

impl<M> Default for BinDelta<M> {}

impl<M> BinDelta<M> {
    pub fn new() -> Self;
    pub fn replace(&mut self, object: BinObject<M>) -> Option<BinObject<M>>;
    pub fn remove(&mut self, path_hash: impl Into<BinHash>) -> Option<BinObject<M>>;
    pub fn append(&mut self, object: BinObject<M>) -> Option<BinObject<M>>;
    pub fn set_dependencies(&mut self, dependencies: impl IntoIterator<Item = impl Into<String>>);
    pub fn replacement(&self, path_hash: impl Into<BinHash>) -> Option<&BinObject<M>>;
    pub fn is_removed(&self, path_hash: impl Into<BinHash>) -> bool;
    pub fn appended(&self) -> indexmap::map::Values<'_, BinHash, BinObject<M>>;
    pub fn dependencies(&self) -> Option<&[String]>;
    pub fn is_empty(&self) -> bool;
}

impl<R: io::Read + io::Seek, M: Default + Clone> BinStream<R, M> {
    /// Writes the base with `delta` applied.
    pub fn write_patched<W: io::Write>(&mut self, delta: &BinDelta<M>, out: &mut W)
        -> Result<(), Error>;
}
```

## Invariants

The rules are `bin-streaming.md` [section 10.3](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/bin-streaming.md#s10.3). The ones a reviewer checks the output against:

- **Untouched means bit-identical.** An object the delta does not name is never deserialized; its bytes are copied from `byte_range()`.
- **Current-format output.** Every output writes version 3 and current property-kind numbering.
- **A legacy base refuses, read or unread, before output.** Every base object is structurally validated. Legacy numbering fails with `Error::DeltaLegacyNumbering`, whose message names `into_bin()` + `Bin::to_writer`.
- **A delta names its base.** A replaced or removed hash the base does not hold is `Error::DeltaMissingObject`; an appended hash the output also holds is `Error::DeltaDuplicateObject`. Both before any byte reaches `out`.

ADR-0016 records why a delta write and not a whole-file transcode.

Nothing here is blocked. The repair flow of `bin-streaming.md` [section 10.1](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/bin-streaming.md#s10.1) also takes the mutable walk (#237), which depends on nothing here either.

- [ ] An empty delta writes the same version-3 bytes as the eager writer from versions 1, 2 and 3; current-format corpus chunks reproduce byte for byte
- [ ] A one-property edit re-reads equal to the same edit applied to the eager tree, and every other object's bytes are unchanged
- [ ] A version-1/2 base saves at version 3, with or without a dependency edit
- [ ] Removing, replacing with a different class, and appending objects update the class table and counts consistently (round-trip)
- [ ] A missing replaced or removed hash and a duplicate appended hash raise their errors and write nothing
- [ ] A legacy base, read or unread, returns the refusal error before output; malformed untouched objects also fail before output
