---
issue: 219
title: "ValuePath: addressing a position in a bin by hash"
labels: crate:ltk_meta, enhancement, format:bin, area:api
---

Part of #218 (design: `docs/design/ptch-property-patches.md` [section 11](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/ptch-property-patches.md#s11)).
The address type a merge or a diff reports positions with, and its fallible conversion to the
client's path language.

A `PropertyPath` is text, and `Segment::name_hash` is FNV-1a of that text, so writing one needs the
property's plaintext name. A bin stores name hashes only, so a walk over two bins cannot always
spell where it is. Every later ticket depends on this decision, which is why it goes first.

## Proposed surface

```rust
/// Where a merge or diff touched, addressed by hash. Total: every position in a value tree has
/// one. Not a client path - it may name a field whose plaintext is unknown, and it is never
/// written to a file.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ValuePath(Vec<Step>);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Step {
    /// A property or field, by name hash.
    Field(BinHash),
    /// A container element, or the value inside an Option.
    Index(u32),
    /// A map entry, by its key value.
    Key(PropertyValueEnum),
}

impl ValuePath {
    /// The client path naming the same position, if every field hash has a known name.
    ///
    /// # Errors
    ///
    /// [`Unnameable`] naming the first field hash `names` could not spell, or the first `Key`
    /// step whose key kind has no `{...}` literal.
    pub fn to_property_path(&self, names: &dyn FieldNames) -> Result<PropertyPath, Unnameable>;
}

/// Plaintext for bin field hashes. `ltk_ritobin::hashes` already has an implementation of this
/// shape; this is the smaller trait `ltk_meta` can own without depending on it.
pub trait FieldNames {
    fn field(&self, hash: BinHash) -> Option<&str>;
}
```

## Rationale

**D19 (ADR-0005): a separate type, not an escape in the path grammar.** `is_name_char` accepts everything but
`.[]{}()` and control characters, so `0x1234abcd` and `#1234abcd` are legal `PropertyPath` names
today and hash as their own text. Any hash escape is therefore a breaking change to
`PropertyPath::new`, and it produces text the client would misread. Keeping the two apart also
keeps `PropertyPath`'s promise: every one of them is something the client can resolve.

What earns the second type is **totality**. Every position in a value tree has a `ValuePath`,
including positions that have no name and never will: an element of a container, an entry of a
map. A report addressed in `PropertyPath` would have to give up on exactly the positions a user
most needs told about. `PropertyPath` is the export language, `ValuePath` is the reporting
language.

Not the justification: sparing a caller a hashtable. `lol-meta-classes` resolves field names, and
`ltk-manager` gates its check, sweep and repair on `ModLibrary::hashtables_ready()` already, so a
name table is present wherever this runs.

- [ ] `ValuePath` and `Step` derive the common traits, and `Display` renders a readable form
      (`4a47c414.Position[0]`) for logs
- [ ] `to_property_path` produces a path that `resolve` lands on the same value with, for every
      position in a fixture tree with a complete name table
- [ ] `to_property_path` reports the first unnameable field hash rather than the last
- [ ] A `Key` step whose key kind has no JSON literal (per `key_as`) is reported, not silently
      rendered
- [ ] `FieldNames` has a blanket implementation for the map types a caller is likely to hold
