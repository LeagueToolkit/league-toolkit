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
/// Beside the steps it keeps the **class context**: for each `Field` step, the class hash of
/// the node the field was read on, which is what a name table is asked with (ADR-0012). A
/// class of 0 means unknown. The context is not part of the address: two paths with the same
/// steps are equal whatever their classes, and the hash form does not print them.
///
/// `PartialEq`, `Eq` and `Hash` are written by hand over the steps alone (W16).
#[derive(Clone, Debug, Default)]
pub struct ValuePath { /* steps: Vec<Step>, classes: Vec<BinHash>, one per Field step */ }
impl PartialEq for ValuePath { /* steps only */ }
impl Eq for ValuePath {}
impl Hash for ValuePath { /* steps only */ }

/// One step from a node toward a position inside it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Step {
    /// A property of a node, by the field's name hash.
    Field(BinHash),
    /// A container element by position, or the value of a present optional, which is always 0.
    Index(usize),
    /// A map entry, by its key.
    Key(MapKey),
}

/// A map key, owned and metadata-free: every kind `Kind::is_valid_map_key` admits.
///
/// Floats are held as their bit patterns so the key is `Eq` and `Hash`: two keys are equal
/// when the file would write the same bytes for them. The client's names for the tags (W19).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MapKey {
    None,
    Bool(bool),
    I8(i8), U8(u8), I16(i16), U16(u16), I32(i32), U32(u32), I64(i64), U64(u64),
    F32(FloatBits),
    Vector2([FloatBits; 2]), Vector3([FloatBits; 3]), Vector4([FloatBits; 4]),
    Matrix44([FloatBits; 16]),
    Color(Color),
    String(String),
    Hash(BinHash),
    File(WadHash),
}

/// An `f32` by its bits: `Eq` and `Hash`, and equal exactly when the wire bytes are.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FloatBits(u32);
impl FloatBits { pub fn new(value: f32) -> Self; pub fn get(self) -> f32; }

impl MapKey {
    pub fn kind(&self) -> Kind;
    /// A leaf as a key, or `None` for `Link` and `Flag`, which no map is keyed by.
    pub fn from_leaf(leaf: Leaf<'_>) -> Option<Self>;
    pub fn to_value(&self) -> PropertyValueEnum;
}
impl<M> TryFrom<&PropertyValueEnum<M>> for MapKey { type Error = Error; /* InvalidKeyType */ }

impl ValuePath {
    pub fn new() -> Self;
    pub fn steps(&self) -> &[Step];
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;

    /// Appends a field step, recording `class` - the class hash of the node `field` is on -
    /// in the class context. Pass 0 when it is not known.
    pub fn push_field(&mut self, field: BinHash, class: BinHash);
    pub fn push_index(&mut self, index: usize);
    pub fn push_key(&mut self, key: MapKey);
    /// Appends `step` with no class: a `Field` records 0.
    pub fn push(&mut self, step: Step);
    /// Removes the last step, and its class if it was a field.
    pub fn pop(&mut self) -> Option<Step>;

    /// Every field step with its class, in order; `None` where the class is unknown.
    pub fn fields(&self) -> Fields<'_>;

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
/// Steps only; every field's class is unknown.
impl FromIterator<Step> for ValuePath {}
impl Extend<Step> for ValuePath {}
/// The iterator behind [`ValuePath::fields`]: `(field, class)` pairs.
#[derive(Clone, Debug)]
pub struct Fields<'a> { /* ... */ }
impl Iterator for Fields<'_> { type Item = (BinHash, Option<BinHash>); }
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
    /// `names` has no plaintext for `field`; `class` is the context it was asked with.
    Field { field: BinHash, class: Option<BinHash> },
    /// A map key of a kind the path grammar has no literal for.
    Key(Kind),
}

/// Plaintext for the hashes a `ValuePath` carries.
pub trait FieldNames {
    /// The plaintext of `field`, if known, given the class of the node it was read on.
    ///
    /// A table keyed by field alone ignores `class`; a table keyed by class - a meta class
    /// dump - needs it and answers nothing for `None`. Either way the name must hash back to
    /// `field` under `BinHash::hash_str`.
    fn field(&self, field: BinHash, class: Option<BinHash>) -> Option<Cow<'_, str>>;

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

**W4 (ADR-0012): the class context rides beside the steps.** The tables a consumer holds are
keyed by class (`lol-meta-classes`, the manager's migration tables), so naming a field takes the
class it was read on, and by the time a report is rendered the tree that would say is gone. The
class is context for the name table and not part of the address: `Step::Field` carries the field
alone, two paths with the same steps are equal whatever their classes (W16), the hash form does
not print them, and a path built from steps alone has none (W15).

**W17: the context is the concrete class.** The object's class hash, or the class a `Struct` or
`Embedded` carries - for a pointer, the class the client constructs, which may be a descendant of
the declared one. A field may be declared on a base class, and a class-keyed table walks the base
chain itself, as the client does from `cls+56`.

**W18: a `Key` step's client path is unattested.** The `{key}` literal is JSON by D10; the
reversing notes' worked example writes bare text, and no shipped record uses one. The round trip
attests this crate's resolver, not the client's.

**W5: `MapKey`, not the value model.** The address type names no `PropertyValueEnum`; a key is
an owned, metadata-free `MapKey` with floats as bits, so `ValuePath` is `Eq` and `Hash` and a
repair can key its findings on it directly.

**Rendering** is the `PropertyPath` grammar with hex where a name is unknown, one table for all
three forms in `value-walk.md` [section 4.2](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/value-walk.md#s4.2). `to_property_path` writes a `Hash` key as its raw
decimal value even when a name is known (W11): the value is what is attested and the client
coerces a number either way.

- [ ] `ValuePath`, `Step`, `MapKey`, `NamedPath`, `Unnameable` and `UnnameableKind` carry the
      traits above; two `F32` keys with the same bits are equal and hash the same; `Display` on
      `ValuePath` writes the hash form of every row of the rendering table
- [ ] `MapKey::try_from` accepts every kind `Kind::is_valid_map_key` admits and rejects the rest
      with `InvalidKeyType`; `to_value` round-trips
- [ ] `to_property_path` produces a path that `Bin::resolve` lands on the same value with, for
      every position in a fixture tree with a complete name table (AC-7)
- [ ] `to_property_path` reports the first unnameable step rather than the last, and a `Key` step
      whose kind has no literal is reported as `UnnameableKind::Key`, not silently rendered
- [ ] `to_named` spells every hash `names` knows and leaves the rest as hex; `named` plus `unnamed`
      equals the count of field steps plus hash-kind keys
- [ ] `push_field`, `pop`, `push` and `FromIterator` keep one class per field step; `fields()`
      yields `None` for a class of 0; two paths with equal steps and different classes are equal
- [ ] `FieldNames` is implemented for `()`, the two `HashMap` shapes, `&T`, and
      `ltk_ritobin::hashes::HashMapProvider`
