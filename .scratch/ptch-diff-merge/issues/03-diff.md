---
issue: 221
title: "Bin::diff: two bins into a BinOverride"
labels: crate:ltk_meta, enhancement, format:bin, area:api, blocked
---

Part of #218 (design: `docs/design/ptch-property-patches.md` [section 12](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/ptch-property-patches.md#s12); requirement PRD-001 FR-10).
The difference between two bins as a patch, and every place the record language could not carry
it. The consumer is `league-mod`'s conversion of a mod's replaced bins into a `.ptch` and into
game-data declarations (`league-mod` `docs/research/bin-diff-to-declarations.md`): the patch is
its `.ptch` export, and `DiffReport::lifted` is what its declaration renderer rewrites.

## Proposed surface

```rust
/// How a diff treats what the edit leaves out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub struct DiffOptions {
    /// An object in the base and not in the edit goes on [`BinOverride::deleted`].
    ///
    /// Off by default: a mod that omits an object is not asking for it to be deleted (ADR-0012).
    /// A tool authoring a deliberate patch turns it on.
    pub deletions: bool,
}

/// A place the record language could not carry what the diff found there.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Lift {
    /// The edit adds `keys` entries to the map at `at`. No record inserts a map entry.
    MapInsert { object_hash: BinHash, at: ValuePath, keys: usize },
    /// No client path spells `at`: a field on it has no name, or a key has no literal.
    Unnameable { object_hash: BinHash, at: ValuePath, cause: Unnameable },
    /// The two sides hold different shapes at `at`. No record changes a value's shape.
    Mismatch { object_hash: BinHash, at: ValuePath },
}
impl Lift {
    pub fn object_hash(&self) -> BinHash;
    pub fn at(&self) -> &ValuePath;
}
// Display: "01000001 1e6ba0c4: 2 map entries inserted"

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct DiffReport {
    /// Records emitted.
    pub records: usize,
    /// Objects taken whole into [`BinOverride::objects`], in the edit's order.
    pub objects: Vec<BinHash>,
    /// Objects put on the delete list. Empty unless [`DiffOptions::deletions`].
    pub deleted: Vec<BinHash>,
    /// Dependencies the edit declares and the base does not.
    pub dependencies: Vec<String>,
    /// Every escalation, in walk order.
    pub lifted: Vec<Lift>,
}
// Display: "3 records, 1 objects, 0 deleted, 0 dependencies, 2 lifted"

impl<M: Clone + PartialEq> Bin<M> {
    /// The patch that turns this bin into `edited`, as far as records can say it.
    pub fn diff(&self, edited: &Self, names: &dyn FieldNames) -> (BinOverride<M>, DiffReport);
    /// See [`DiffOptions`].
    pub fn diff_with(&self, edited: &Self, names: &dyn FieldNames, options: &DiffOptions)
        -> (BinOverride<M>, DiffReport);
}
```

## Rationale

**The walk is merge's.** Diff descends two bins by the table of [section 10.1](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/ptch-property-patches.md#s10.1) and writes a
record where merge would replace or insert. A container replaces whole (D22), a Pointer of another
class is a plain record (D11), and a map entry matches by its `MapKey` (D36).

**Three things escalate a record** to an ancestor, or to the whole object at the root: a map entry
the edit adds (`Lift::MapInsert`), a change of shape the type rule skips (`Lift::Mismatch`), and a
position no client path spells (`Lift::Unnameable`). An object of another class goes whole into
`BinOverride::objects` with a mismatch at its root (D35).

**D33 (ADR-0019): an escalated record carries the base merged with the edit.** The invariant holds
whatever was lifted:

```text
base.diff(edited).apply(base)  ==  base.merge(edited)      on objects
```

A lift marks a record that carries more than the difference, and overwrites what another base
holds inside it.

**D34: the object sits beside the position.** Every `Lift` carries `object_hash`, and a report on
a `Bin` names the object of each position.

**D21: deletions are off by default.** A mod that omits an object is not asking for its deletion.

Blocked by #219 (the trail and `ValuePath::to_property_path`) and #220 (the merged value an
escalation carries)

- [ ] A changed leaf and a property the base lacks are one record each, at their own paths
- [ ] A changed map entry is a record at its key; an added one lifts the map, and the map's record
      keeps the base's other keys
- [ ] An unnamed field lifts to its nearest named ancestor, and with no names at all to the object
- [ ] A changed shape lifts to the parent; a changed object class takes the object whole
- [ ] A new object is taken whole; an omitted one goes on the delete list only with
      `DiffOptions::deletions`; an edit-only dependency is reported and not emitted
- [ ] Equal bins diff to an empty patch and an empty report
- [ ] A key the edit repeats diffs as the merge applies it; in a float-keyed map holding `0.0` and
      `-0.0`, a difference at the later key lifts the map
- [ ] Siblings under one unnamed field each lift at their own position
- [ ] On generated pairs, with a complete, a partial and an empty name table: the invariant holds,
      `check` and `apply` are clean, and an unspellable segment is lifted once
- [ ] With a complete name table only a map insert or a shape lifts, and no record writes what the
      base already holds
- [ ] `uiflipped` applied to `uibase` and diffed back writes nothing outside its records and
      applies as `uiflipped` does
- [ ] Corpus: every shipped `PTCH`, applied and diffed back, equals the merge, and writes nothing
      outside its records but the whole container one of them indexes into
