---
issue: 219
title: "ValuePath: addressing a position in a bin by hash"
labels: crate:ltk_meta, enhancement, format:bin, area:api
---

Part of #218 (design: `docs/design/value-walk.md` [section 4](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/value-walk.md#s4)).
The address type a merge, a diff or a walk reports positions with, its two text forms, and its
fallible conversion to the client's path language.

A `PropertyPath` is text, and `Segment::name_hash` is FNV-1a of that text, so writing one needs
the property's plaintext name. A bin stores name hashes only, so a walk cannot always spell where
it is. Every later ticket depends on this type, which is why it goes first.

## Proposed surface

In `ltk_meta::path`, beside `PropertyPath`:

```rust
/// Where a walk is inside one object, addressed by hash and by position.
///
/// Total: every position in a value tree has one. Not a client path - it may name a field
/// whose plaintext is unknown - and never written to a file. The object it is inside is
/// carried beside it, never in it.
///
/// `PartialEq` and not `Eq` or `Hash`, because a map key may be an `f32`. The form to key on
/// is the hash form, `to_string()`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ValuePath(Vec<Step>);

/// One step from a node toward a position inside it.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// A property of a node: the class hash the node carries, and the field's name hash.
    ///
    /// The class rides on every field step because naming a field takes the class it is on
    /// (ADR-0012). It is context for a name table, not part of the rendered address.
    Field { class: BinHash, field: BinHash },
    /// A container element by position, or the value of a present optional, which is always 0.
    Index(usize),
    /// A map entry, by its key. Metadata is dropped; the key is otherwise the one the map holds.
    Key(PropertyValueEnum),
}

impl ValuePath {
    pub fn new() -> Self;
    pub fn steps(&self) -> &[Step];
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn push(&mut self, step: Step);
    pub fn pop(&mut self) -> Option<Step>;

    /// The client path naming the same position, if every field has a name and every key a
    /// literal.
    ///
    /// # Errors
    ///
    /// [`Unnameable`] at the first step that cannot be spelled: a field `names` has no plaintext
    /// for, or a key whose kind has no `{...}` literal.
    pub fn to_property_path(&self, names: &dyn FieldNames) -> Result<PropertyPath, Unnameable>;

    /// The path for reading: every hash `names` can spell, spelled; the rest left as hex.
    pub fn to_named(&self, names: &dyn FieldNames) -> NamedPath;
}

impl fmt::Display for ValuePath { /* the hash form */ }
impl FromIterator<Step> for ValuePath {}
impl Extend<Step> for ValuePath {}
impl<'a> IntoIterator for &'a ValuePath { type Item = &'a Step; /* ... */ }
impl IntoIterator for ValuePath { type Item = Step; /* ... */ }

/// A best-effort readable rendering of a [`ValuePath`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedPath {
    /// The text. Every hash a table could spell is a name; every other is hex, exactly as the
    /// hash form writes it.
    pub text: String,
    /// Hashes the table spelled: field hashes and hash-kind keys.
    pub named: usize,
    /// Hashes it could not.
    pub unnamed: usize,
}

impl NamedPath {
    /// Every hash was spelled.
    pub fn is_complete(&self) -> bool;
}

impl fmt::Display for NamedPath { /* `text` */ }

/// A position `to_property_path` could not spell.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{kind} (step {step})")]
pub struct Unnameable {
    /// Index into `ValuePath::steps` of the first step that could not be spelled.
    pub step: usize,
    pub kind: UnnameableKind,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum UnnameableKind {
    /// `names` has no plaintext for `field` on `class`.
    Field { class: BinHash, field: BinHash },
    /// A map key of a kind the path grammar has no literal for.
    Key(Kind),
}

/// Plaintext for the hashes a `ValuePath` carries.
pub trait FieldNames {
    /// The plaintext of `field` on `class`, if known.
    ///
    /// A table keyed by field alone ignores `class`; a table keyed by class - a meta class
    /// dump - needs it. Either way the name must hash back to `field` under `BinHash::hash_str`.
    fn field(&self, class: BinHash, field: BinHash) -> Option<Cow<'_, str>>;

    /// The plaintext behind a `Hash`-kind map key, if known. Named form only.
    fn hash(&self, hash: BinHash) -> Option<Cow<'_, str>> { None }
}

impl FieldNames for () {}
impl FieldNames for HashMap<BinHash, String> {}
impl FieldNames for HashMap<(BinHash, BinHash), String> {}
impl<T: FieldNames + ?Sized> FieldNames for &T {}
```

`ltk_ritobin::hashes::HashMapProvider` implements `FieldNames`: `field` through its field table
ignoring `class`, `hash` through its hash table.

## Rationale

**D19 (ADR-0005): a separate type, not an escape in the path grammar.** `0x1234abcd` and
`#1234abcd` are legal `PropertyPath` names today and hash as their own text, so any hash escape
breaks `PropertyPath::new` and produces text the client misreads. What earns the second type is
**totality**: every position in a value tree has a `ValuePath`, including the ones a report most
needs to name and a `PropertyPath` cannot - a container element, a map entry.

**W4 (ADR-0012): the class rides on the field step.** The tables a consumer holds are keyed by
class (`lol-meta-classes`, the manager's migration tables), so naming a field takes the class it
was read on, and by the time a report is rendered the tree that would say is gone. The class is
context for the name table and is not printed: the hash form of a path is the same whatever
class it went through.

**W5: `PartialEq` only.** `Kind::is_valid_map_key` admits `F32` and the vector kinds, so a `Key`
step can hold a float. The hash form is the total, stable, `Eq` key; a consumer that keys a map on
an address uses `to_string()`.

**Rendering** is the `PropertyPath` grammar with hex where a name is unknown, one table for all
three forms in `value-walk.md` [section 4.2](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/value-walk.md#s4.2). `to_property_path` writes a `Hash` key as its raw
decimal value even when a name is known (W11): the value is what is attested and the client
coerces a number either way.

- [ ] `ValuePath`, `Step`, `NamedPath`, `Unnameable` and `UnnameableKind` derive the traits above,
      and `Display` on `ValuePath` writes the hash form of every row of the rendering table
- [ ] `to_property_path` produces a path that `Bin::resolve` lands on the same value with, for
      every position in a fixture tree with a complete name table (AC-7)
- [ ] `to_property_path` reports the first unnameable step rather than the last, and a `Key` step
      whose kind has no literal is reported as `UnnameableKind::Key`, not silently rendered
- [ ] `to_named` spells every hash `names` knows and leaves the rest as hex; `named` plus `unnamed`
      equals the count of field steps plus hash-kind keys
- [ ] `FieldNames` is implemented for `()`, the two `HashMap` shapes, `&T`, and
      `ltk_ritobin::hashes::HashMapProvider`
