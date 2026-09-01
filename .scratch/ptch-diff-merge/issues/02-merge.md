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
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct MergeReport<M = NoMeta> {
    /// Objects taken whole from the edit because the base had no such hash.
    pub objects_added: Vec<BinHash>,
    /// Objects that existed on both sides and were combined.
    pub objects_merged: Vec<BinHash>,
    /// Every leaf the edit overwrote, with the value the base held there.
    pub replaced: Vec<Replaced<M>>,
    /// Properties the base object did not have.
    pub inserted: usize,
    /// Map entries the base map did not have.
    pub keys_inserted: usize,
}

/// One leaf the edit overwrote.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Replaced<M = NoMeta> {
    /// Where it happened.
    pub at: ValuePath,
    /// What the base held. Moved out of the base rather than cloned, so recording every
    /// replacement costs nothing a merge was not already paying.
    pub was: PropertyValueEnum<M>,
    /// Whether the two sides held different kinds, or a Struct of a different class.
    pub mismatched: bool,
}

impl<M: Clone + PartialEq> Bin<M> {
    /// Layers `edited` over this bin, in place: ADR-0012's merge.
    pub fn merge(&mut self, edited: &Self) -> MergeReport<M>;
}
impl<M: Clone + PartialEq> BinObject<M>         { /* the same, over properties */ }
impl<M: Clone + PartialEq> values::Struct<M>    { /* the same */ }
impl<M: Clone + PartialEq> PropertyValueEnum<M> { /* the same, one value against one value */ }
```

## The descent

`edited` wins at every leaf it reaches; anything only the base has survives. Absence in `edited` is
never a difference - that is the whole of ADR-0012, and the record language has no way to express a
removal in any case.

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
| any | different kind | replace, and count it in the report |

Key equality is `key_eq`, so metadata is ignored there. A `Map` merge is quadratic on entry counts
unless the walk indexes one side first; shipped maps reach the low thousands of entries, so index
the base side. Dependencies merge as a union: the base's list in its order, then anything only
`edited` has.

**D22 Containers replace whole (ADR-0004).** No element-wise merge, no LCS: ADR-0012's semantics are the
client's, a list has no key to combine by, and a positional merge invents a meaning the format does
not have.

**D24 Metadata is out of scope for value comparison.** `M: PartialEq` decides "different value", so
a metadata varying per occurrence makes every leaf differ. `ltk_ritobin`'s
`PropertyValueEnum<Span>` is that case: map it through `no_meta()` first.

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

- [ ] `merge` is idempotent: `base.merge(e).merge(e)` equals `base.merge(e)` (property test)
- [ ] `merge` is absorbing: `base.merge(base)` equals `base`, and the report records nothing
- [ ] A base-only property, and a base-only map key, survive a merge that does not name them
- [ ] An edit-only map key is inserted, in the edit's order, after the base's entries
- [ ] Every replacement is reported with the base's old value, moved rather than cloned
- [ ] A kind mismatch replaces whole and is reported with `mismatched: true`
- [ ] A `String` value merged over a `File` value of the same field reports one mismatch (the
      16.17 migration case)
- [ ] The ADR-0012 specimen reduced to a fixture: base-only map keys restored, edit's own bindings
      intact, edit's new keys present
- [ ] Dependencies merge as a union with the base's order preserved
