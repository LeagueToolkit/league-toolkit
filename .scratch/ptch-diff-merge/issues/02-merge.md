---
issue: 220
title: "Bin::merge: layer one bin over another"
labels: crate:ltk_meta, enhancement, format:bin, area:api, blocked
---

Part of #218 (design: `docs/design/ptch-property-patches.md` [section 10](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/ptch-property-patches.md#s10)).
The operation `ltk-manager` ADR-0012 names: a mod's content layered over the game's copy, objects
combined field by field and maps key by key, so that what the mod does not carry forward survives.
This is the ticket with a consumer waiting.

## Proposed surface

```rust
/// What a merge did: what it overwrote, and what it added.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct MergeReport<M = NoMeta> {
    /// Objects taken whole from the edit: the base had no object with the hash.
    pub objects_added: Vec<BinHash>,
    /// Objects on both sides with the same class, combined property by property.
    pub objects_merged: Vec<BinHash>,
    /// Objects on both sides with different classes, each as the base held it.
    pub objects_replaced: Vec<BinObject<M>>,
    /// Every value the edit overwrote inside a combined object, with the value the base held.
    pub replaced: Vec<Replaced<M>>,
    /// Properties the base did not have, inserted from the edit.
    pub inserted: usize,
    /// Map entries the base did not have, appended from the edit.
    pub keys_inserted: usize,
    /// Dependencies the base did not declare, appended from the edit.
    pub dependencies_added: Vec<String>,
}
impl<M> Default for MergeReport<M> {}

impl<M> MergeReport<M> {
    /// Whether the merge left the base as it was: nothing added, replaced or inserted.
    pub fn is_unchanged(&self) -> bool;
}
// Display: "1 added, 2 merged, 0 replaced; 3 values replaced (1 mismatched), 4 inserted, 5 keys inserted, 0 dependencies added"

/// One value the edit overwrote.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Replaced<M = NoMeta> {
    /// The object the value is in. 0 for a merge of a `Struct` or a value.
    pub object_hash: BinHash,
    /// Where the value is, inside that object.
    pub at: ValuePath,
    /// What the base held. Moved out of the base, never cloned.
    pub was: PropertyValueEnum<M>,
    /// Whether the two sides held different shapes.
    pub mismatched: bool,
}
// Display: "01000001 1e6ba0c4 (mismatched)"

impl<M: Clone + PartialEq> Bin<M> {
    /// Layers `edited` over this bin, in place. A merge never refuses.
    pub fn merge(&mut self, edited: &Self) -> MergeReport<M>;
}
impl<M: Clone + PartialEq> BinObject<M>         { /* the same, over properties */ }
impl<M: Clone + PartialEq> values::Struct<M>    { /* the same, object_hash 0 */ }
impl<M: Clone + PartialEq> PropertyValueEnum<M> { /* the same, one value against one value */ }
```

## The descent

`edited` wins at every leaf it reaches; anything only the base has survives. Absence in `edited` is
never a difference - that is the whole of ADR-0012, and the record language has no way to express a
removal in any case.

An object only `edited` has is added whole. An object on both sides with the same class combines
property by property, and one with different classes is replaced whole and reported in
`objects_replaced` (D35). Below an object:

| base | edited | action |
|---|---|---|
| property absent | any | insert `edited`'s value |
| Struct or Embed, same class | same kind, same class | recurse field by field |
| Struct or Embed, different class | any | replace |
| Struct with class 0 (a null pointer) | any | replace |
| Map, same key and value kinds | Map | recurse on common keys, append `edited`'s new ones in its order, keep base-only keys |
| Map, different key or value kinds | any | replace |
| Container, UnorderedContainer | any | replace whole (D22) |
| Optional, both present | Optional | recurse into the contained value |
| Optional, either absent | Optional | replace |
| any leaf kind | equal value | nothing |
| any leaf kind | different value | replace |
| any | different kind | replace, with `mismatched` set |

A map entry matches by its `MapKey`, metadata ignored and a float key by its bits, and the base's
keys are indexed once per map (D36). A key the edit repeats merges over its earlier occurrence. Dependencies merge as a union: the base's list in its order,
then anything only `edited` has. Every replacement names its object beside its position (D34).

**D22 Containers replace whole (ADR-0004).** No element-wise merge, no LCS: ADR-0012's semantics are the
client's, a list has no key to combine by, and a positional merge invents a meaning the format does
not have.

**D24 Value comparison is `M`'s `PartialEq`.** A metadata varying per occurrence makes every leaf
differ: `ltk_ritobin`'s `PropertyValueEnum<Span>` goes through `no_meta()` first. A float leaf
compares with `==`: a `NaN` differs from itself, and `-0.0` equals `0.0`.

## Why `Replaced::mismatched` is the field that matters

An exact-tag mismatch between a mod's value and the game's is the signature of a **type
migration**. Riot performs those in place: 337 times in three years, then 327 in the single 16.17
`String` -> `File` patch. The client's tag rule is exact byte equality with no coercion, and a
value whose tag does not match is consumed and discarded with no error and no log line, leaving
the field at whatever the object's constructor put there.

Measured on one champion WAD across that patch: 0 `File` values become 3,778 across 10 fields, led
by `StaticMaterialShaderSamplerDef.texturePath` (1,826) and
`AnimationResourceData.mAnimationFilePath` (1,595) - retexturing and adding a custom animation,
the two most common things a skin mod does. A mod predating the migration loses both, silently.
Merging writes the mod's stale value through, reproducing the loss; this flag is what lets a
caller catch it first, and a caller holding a meta class dump can name the migration exactly.

Blocked by #219

- [ ] `merge` is idempotent: `base.merge(e).merge(e)` equals `base.merge(e)`, and the second report `is_unchanged` (property test)
- [ ] `merge` is absorbing: `base.merge(base)` equals `base`, and the report `is_unchanged` (property test)
- [ ] A base-only property, and a base-only map key, survive a merge that does not name them
- [ ] An edit-only map key is inserted, in the edit's order, after the base's entries
- [ ] Every replacement is reported at its object and position, with the base's old value: its `at`, spelled as a client path, resolves to `was` in the base (property test)
- [ ] A kind mismatch replaces whole and is reported with `mismatched: true`
- [ ] A `String` value merged over a `File` value of the same field reports one mismatch (the
      16.17 migration case)
- [ ] The ADR-0012 specimen reduced to a fixture: base-only map keys restored, edit's own bindings
      intact, edit's new keys present
- [ ] Dependencies merge as a union with the base's order preserved
- [ ] An object of another class is replaced whole and reported in `objects_replaced`; a node of another class is one mismatched replacement; a container replaces whole; an optional combines what it holds
- [ ] A map of other kinds replaces whole; a float key matches by its bits; a key the edit repeats
      merges over its earlier occurrence; a map breaking its declared kinds replaces whole without a
      panic
- [ ] A merge that only adds a dependency reports it in `dependencies_added` and is not unchanged
