# Walking a bin's value tree in `ltk_meta`

The API spec for `ltk_meta::walk` and `ltk_meta::path::ValuePath`: one read-only traversal over
every node of an object, driven by a visitor with a prune, and the address type a position in
that tree is reported with.

**This document states what is true now.** Where the code and a section here disagree, the section
is the bug and gets edited. Two things it does not hold:

- **Why this exists, who asks for it, and what it must do** -
  `docs/prd/001-ptch-property-patches.md` (FR-7, FR-8, FR-12, FR-13), cited here as FR-N.
- **Why an option was chosen over the alternatives it beat** - ADR-0005, ADR-0012 and ADR-0013,
  cited from the rules in [section 8](#s8).

Designed and not yet built, tracked as #219 (`ValuePath`) and #225 (the walk).

## <a id="s1"></a>1. Summary

Every consumer that reads a bin for what it contains ends up writing the same recursion: match
the six recursive variants of `PropertyValueEnum`, keep a stack of where you are, render that
stack as text when something is worth reporting. `ltk-manager` has two copies of it today, and
`Bin::merge` and `Bin::diff` (`ptch-property-patches.md` [section 10](ptch-property-patches.md#s10)
and [section 12](ptch-property-patches.md#s12)) each need one more. This module owns it once.

The module holds:

- **A predicate** that says which values can hold a node: `Kind::is_node` and
  `PropertyValueEnum::holds_node` ([section 3](#s3)).
- **`ValuePath`**, the address of a position inside one object, by hash and by position, with a
  stable hex rendering and a best-effort named one ([section 4](#s4)).
- **The walk**: `BinObject::walk`, `Bin::walk` and `BinOverride::walk`, which call a `Visitor`
  once per node in a fixed order, ask it before entering each property, and carry a `Trail` that
  renders to a `ValuePath` only when asked ([section 5](#s5)).

Everything a consumer builds on top - running several visitors over one walk, deciding what to
report, keeping a budget - stays with the consumer (ADR-0013).

## <a id="s2"></a>2. Vocabulary

Every term this document uses in a specific sense.

**The tree**

- **object** - a top-level `BinObject`: a path hash, a class hash and properties.
- **node** - an object, or a nested `Struct` or `Embedded` value whose class hash is not 0. A node
  is the thing a visitor is called on: it has a class and properties. A `Struct` with class 0 is
  the client's null pointer; it has no properties and is not a node.
- **property** - one `(field hash, value)` pair of a node, in the node's property order.
- **leaf** - a value that holds no node and cannot hold one: every primitive, `String`, `Hash`,
  `WadChunkLink`, `ObjectLink`, `BitBool`, and a container, optional or map whose item kind is not
  a node kind. A `Kind` is a **node kind** when it is `Struct` or `Embedded`.

**Moving through it**

- **walk** - the traversal: for one object, visit the root, then for each property the visitor
  enters, descend the value, visiting every node reached. Pre-order, in file order, exactly once
  per node.
- **visitor** - the consumer's side of the walk: `visit`, called per node; `enters`, asked per
  property, which is the visitor's prune; `leaves`, told when an entered property is done.
- **enter** and **prune** - the walk enters a property's value when the visitor says so, and
  otherwise skips it whole. A visitor that declines a value is not shown anything beneath it.
- **descend** - the walk crossing a container, optional or map to reach the nodes inside it.
  Descent is never asked about: once a property is entered, every node inside it is visited.

**Addresses**

- **step** - one move from a node toward a position inside it: a field, an index or a map key.
  A step carries no class; the class of each node a path passes through is the path's **class
  context**, kept beside the steps (ADR-0012).
- **trail** - the walk's own record of the steps from the root object to its current position.
  It borrows the tree, allocates nothing per step, and is what a visitor renders an address from.
- **`ValuePath`** - the owned address: the trail's steps, with map keys copied out. Addresses a
  position *inside one object*; the object's hash is carried beside it, never in it (D13 in
  `ptch-property-patches.md` [section 17](ptch-property-patches.md#s17)).
- **hash form** - a `ValuePath` rendered with every field hash as eight hex digits. Stable across
  machines and name tables, and the form to compare and key on.
- **named form** - the same path with every hash a name table can spell replaced by its name.
  Best effort: what the table cannot spell stays hex, and the result says how much it spelled.
- **name table** - anything implementing `FieldNames`: the plaintext behind a field hash on a
  given class, and behind a hash-kind map key.

## <a id="s3"></a>3. Which values hold a node

```rust
impl Kind {
    /// Whether a value of this kind is a node: `Struct` or `Embedded`.
    ///
    /// The other question - which kinds a value model treats as leaves - is
    /// [`Kind::is_primitive`], and the two are not complements: `ObjectLink` and `BitBool`
    /// are neither primitive nor a node.
    pub fn is_node(self) -> bool;
}

impl<M> PropertyValueEnum<M> {
    /// Whether entering this value can reach a node.
    ///
    /// True for a `Struct` or `Embedded` whose class hash is not 0, and for a container,
    /// optional or map whose item kind [`Kind::is_node`]. An empty optional or container of a
    /// node kind answers true: it *can* hold one, and entering it costs nothing.
    pub fn holds_node(&self) -> bool;
}
```

`holds_node` is the default prune ([section 5.1](#s5.1)). `Kind::is_primitive` plays no part in
the walk and a consumer need not rely on its set (W1).

## <a id="s4"></a>4. `ValuePath`

A `PropertyPath` is text, and `Segment::name_hash` is FNV-1a of that text, so writing one needs
the property's plaintext name. A bin stores name hashes only, so a walk cannot always spell where
it is. `ValuePath` is the address that never needs to: it is **total** - every position in a
value tree has one, including the ones that have no name and never will, a container element or
a map entry - and it is never written to a file. `PropertyPath` is the export language, the thing
a patch record carries and the client resolves; `ValuePath` is the reporting language (ADR-0005).

### <a id="s4.1"></a>4.1 The types

```rust
/// Where a walk is inside one object, addressed by hash and by position.
///
/// Total: every position in a value tree has one. Not a client path - it may name a field
/// whose plaintext is unknown - and never written to a file. The object it is inside is
/// carried beside it, never in it.
///
/// Beside the steps it keeps the **class context**: for each `Field` step, the class hash of
/// the node the field was read on, which is what a name table is asked with (ADR-0012). A
/// class of 0 means unknown - no node carries the null class (W2), so the value is free. The
/// context is not part of the address: two paths with the same steps are equal whatever
/// their classes, and the hash form does not print them.
///
/// `PartialEq` is written by hand over the steps alone (W16), and there is no `Eq` or `Hash`,
/// because a map key may be an `f32`. The form to key on is the hash form, `to_string()`.
#[derive(Clone, Debug, Default)]
pub struct ValuePath { /* steps: Vec<Step>, classes: Vec<BinHash>, one per Field step */ }
impl PartialEq for ValuePath { /* steps only */ }

/// One step from a node toward a position inside it.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// A property of a node, by the field's name hash.
    Field(BinHash),
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

    /// Appends a field step, recording `class` - the class hash of the node `field` is on -
    /// in the class context. Pass 0 when it is not known.
    pub fn push_field(&mut self, field: BinHash, class: BinHash);
    pub fn push_index(&mut self, index: usize);
    pub fn push_key(&mut self, key: PropertyValueEnum);
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

impl fmt::Display for ValuePath { /* the hash form, section 4.2 */ }
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
    /// Every hash was spelled. A complete named path names the same position as the
    /// `PropertyPath` that `to_property_path` would produce.
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
```

`Unnameable` follows D30 of `ptch-property-patches.md`: a struct carrying a position and a public
kind. The first unspellable step is reported, not the last, because a caller that wants to lift a
record to the nearest nameable ancestor (`Lift::Unnameable`, `ptch-property-patches.md`
[section 12](ptch-property-patches.md#s12)) needs the first.

### <a id="s4.2"></a>4.2 Rendering

Both text forms share one grammar, which is the `PropertyPath` grammar of
`ptch-property-patches.md` [section 8.1](ptch-property-patches.md#s8.1) with hex where a name
is not known: `.` between fields, `[i]` for an index, `{key}` for a map entry, and the class of a
field step nowhere in the text. A `ValuePath` with no steps renders as the empty string.

| step                            | hash form           | named form, when the table spells it | `to_property_path`                 |
| ------------------------------- | ------------------- | ------------------------------------ | ---------------------------------- |
| `Field`                         | `1e6ba0c4`          | `Position`                           | `Position`; else `Unnameable`      |
| `Index(3)`                      | `[3]`               | `[3]`                                | `[3]`                              |
| `Key` of an integer kind        | `{12}`              | `{12}`                               | `{12}`                             |
| `Key` of `Bool` or `BitBool`    | `{true}`            | `{true}`                             | `{true}`                           |
| `Key` of `F32`                  | `{1.5}`             | `{1.5}`                              | `{1.5}`                            |
| `Key` of `String`               | `{"weapon"}`        | `{"weapon"}`                         | `{"weapon"}`                       |
| `Key` of `Hash`                 | `{1e6ba0c4}`        | `{"Weapon"}`, via `FieldNames::hash` | `{510369988}`, the raw value       |
| `Key` of `WadChunkLink`         | `{00c9fd8f1a2b3c4d}` | `{00c9fd8f1a2b3c4d}`                | `{56855261380033613}`, the raw value |
| `Key` of a vector, `Color`, `Matrix44` | `{(1, 2)}`   | `{(1, 2)}`                           | `Unnameable`, `Key(kind)`          |
| `Key` of `None`                 | `{}`                | `{}`                                 | `Unnameable`, `Key(Kind::None)`    |

Hex is lowercase, zero-padded to the hash's width: eight digits for a `BinHash`, sixteen for a
`WadHash`. A string key is written as a JSON string, escaped as `serde_json` would write it. An
`F32` key is written in Rust's shortest round-trip form. A vector or colour key is its components
in parentheses, comma separated; it is text for a human and nothing parses it.

`to_property_path` writes a `Hash` key as its raw value in decimal rather than as a name, even
when the table has one: a plaintext hashes back to the same value, but it is the value that is
attested, and the literal the client coerces is a number either way
(`ptch-property-patches.md` D10). The path it produces resolves, by
`Bin::resolve`, to the position the walk was at; that is the round trip [section 7](#s7) tests.

### <a id="s4.3"></a>4.3 `FieldNames`

```rust
/// Plaintext for the hashes a `ValuePath` carries.
///
/// `ltk_ritobin::hashes::HashMapProvider` implements this; it is the smaller trait `ltk_meta`
/// can own without depending on it.
pub trait FieldNames {
    /// The plaintext of `field`, if known, given the class of the node it was read on.
    ///
    /// A table keyed by field alone ignores `class`; a table keyed by class - a meta class
    /// dump - needs it and answers nothing for `None`. Either way the name must hash back to
    /// `field` under `BinHash::hash_str`, which is what makes `to_property_path` resolve where
    /// the walk was.
    fn field(&self, field: BinHash, class: Option<BinHash>) -> Option<Cow<'_, str>>;

    /// The plaintext behind a `Hash`-kind map key, if known. Named form only.
    fn hash(&self, hash: BinHash) -> Option<Cow<'_, str>> { None }
}

/// Names nothing: every hash renders as hex.
impl FieldNames for () {}
/// Keyed by field alone; `class` is ignored.
impl FieldNames for HashMap<BinHash, String> {}
/// Keyed by `(class, field)`.
impl FieldNames for HashMap<(BinHash, BinHash), String> {}
impl<T: FieldNames + ?Sized> FieldNames for &T {}
```

The `class` parameter exists because the tables a consumer holds are keyed by class:
`lol-meta-classes` dumps one meta class at a time, and `ltk-manager`'s migration tables name a
field by the class it is on. A name is looked up with the class the walk saw the field on, which
is what the path's class context holds (ADR-0012). A path built without a tree - from steps
alone - asks with `None`, and a field-keyed table still answers.

## <a id="s5"></a>5. The walk

```rust
/// What a walk calls. One `visit` per node, one `enters` per property, one `leaves` per
/// property entered.
pub trait Visitor<M = NoMeta> {
    /// Called at every node the walk reaches, before any of the node's properties is entered.
    fn visit(&mut self, node: &Node<'_, M>);

    /// Whether the walk descends `value`, the property `field` of the node at `at`.
    ///
    /// The prune. Default: [`PropertyValueEnum::holds_node`]. A visitor that answers `false`
    /// is not shown anything beneath the value.
    fn enters(&mut self, field: BinHash, value: &PropertyValueEnum<M>, at: &Trail<'_, M>) -> bool {
        value.holds_node()
    }

    /// Called once every node beneath a property `enters` accepted has been visited.
    ///
    /// Paired with `enters`, so a visitor keeping state per subtree - which visitors are still
    /// active, a depth counter - can pop it. Default: nothing.
    fn leaves(&mut self, field: BinHash, value: &PropertyValueEnum<M>, at: &Trail<'_, M>) {}
}

/// A `&mut V` is a visitor, so a `&mut dyn Visitor<M>` can be passed where one is wanted.
impl<M, V: Visitor<M> + ?Sized> Visitor<M> for &mut V {}

/// One node, as the walk hands it to a visitor.
#[derive(Clone, Copy, Debug)]
pub struct Node<'a, M = NoMeta> { /* object hash, class, &'a properties, &'a Trail */ }

impl<'a, M> Node<'a, M> {
    /// The path hash of the object this node is in, or is.
    pub fn object_hash(&self) -> BinHash;
    /// The class hash this node carries. Never 0.
    pub fn class_hash(&self) -> BinHash;
    /// The node's properties, in file order.
    pub fn properties(&self) -> &'a IndexMap<BinHash, PropertyValueEnum<M>>;
    /// Where the node is: empty at the root.
    pub fn trail(&self) -> &Trail<'a, M>;
    /// Whether this node is the object itself.
    pub fn is_root(&self) -> bool;
    /// The node's address, copied out of the trail. Allocates; call it for a node worth
    /// reporting on.
    pub fn value_path(&self) -> ValuePath where M: Clone;
}

/// The steps from an object's root to the walk's position.
///
/// Borrows the tree - a map key is a reference, never a copy - so descending a map of ten
/// thousand entries allocates nothing. Text is made only by `to_value_path` or `Display`.
#[derive(Debug)]
pub struct Trail<'a, M = NoMeta> { /* Vec<TrailStep<'a, M>> */ }

/// One step of a [`Trail`]. The borrowing form of [`Step`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrailStep<'a, M = NoMeta> {
    Field(BinHash),
    Index(usize),
    Key(&'a PropertyValueEnum<M>),
}

impl<'a, M> Trail<'a, M> {
    pub fn steps(&self) -> &[TrailStep<'a, M>];
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    /// The class of the node each field step was read on, one per `Field` step, in order.
    /// Never 0: the walk always knows.
    pub fn classes(&self) -> &[BinHash];
    /// The owned address: every step copied, keys with their metadata dropped, the class
    /// context carried over.
    pub fn to_value_path(&self) -> ValuePath where M: Clone;
}

impl<M> fmt::Display for Trail<'_, M> { /* the hash form of section 4.2, without building a ValuePath */ }

impl<'a, M: Clone> TrailStep<'a, M> {
    pub fn to_step(&self) -> Step;
}

impl<M> BinObject<M> {
    /// Walks this object: the root, then every node beneath every property `visitor` enters.
    pub fn walk<V: Visitor<M>>(&self, visitor: &mut V);
}

impl<M> Bin<M> {
    /// Walks every object, in file order.
    pub fn walk<V: Visitor<M>>(&self, visitor: &mut V);
}

impl<M> BinOverride<M> {
    /// Walks every embedded object, in file order. Patch records are not walked: a record's
    /// value has no node of its own to stand on, and the objects are content the game loads
    /// whether or not the records apply.
    pub fn walk<V: Visitor<M>>(&self, visitor: &mut V);
}
```

The walk is over the owned tree and cannot fail. It reads a `Bin` the caller already holds, so it
has no I/O and no error; a visitor that wants to stop early declines every further `enters` (W6).

### <a id="s5.1"></a>5.1 Traversal rules

For one object:

1. The object is a node with an empty trail. `visit` is called on it.
2. For each property `(field, value)` of a node, in property order, the walk asks
   `enters(field, value, trail)`. If the answer is `false`, the value is skipped whole and
   nothing beneath it is visited.
3. Otherwise `Field(field)` is pushed, with the node's class hash recorded in the class
   context, and the walk descends `value`:
   - `Struct` or `Embedded` with class hash not 0: a node. `visit`, then rule 2 recurses.
   - `Struct` or `Embedded` with class hash 0: a null pointer. Nothing.
   - `Container` or `UnorderedContainer`: for each item, `Index(i)` is pushed, the item is
     descended, and the step is popped.
   - `Optional` holding a value: `Index(0)` is pushed, the value is descended, the step is
     popped. An optional is indexed rather than stepped through, because that is how the path
     grammar addresses it (D9 in `ptch-property-patches.md`).
   - `Map`: for each entry, `Key(&key)` is pushed, the value is descended, the step is popped.
     Keys are never descended; a map key is a leaf by construction.
   - Anything else holds no node. Nothing.
4. The `Field` step is popped and `leaves(field, value, trail)` is called.

So a visitor sees nodes in pre-order, in file order, each exactly once, and every push is popped
before the walk returns. A default `enters` visits every node in the object; a visitor is only
ever asked about properties, never about the items inside a container, optional or map it chose
to enter.

`Bin::walk` and `BinOverride::walk` apply the above to each object in file order, with the trail
emptied between objects. Nothing carries over from one object to the next.

### <a id="s5.2"></a>5.2 The trail

The trail holds hashes, indices and borrowed keys, never text. A step costs a push. `Display` on
a `Trail` writes the hash form straight from the borrows, and `to_value_path` copies the steps
out; either is what a visitor does for a node it reports on, and neither happens otherwise.

Beside the steps the trail keeps the class context: for each `Field` step, the class hash of
the node the field was read on - the object's class hash at the root, the `Struct` or
`Embedded` class hash below it. It is what a name table is asked with ([section 4.3](#s4.3)),
and `to_value_path` carries it over.

## <a id="s6"></a>6. Where else the trail is used

**From the stream.** The walk takes an owned object, so a consumer sweeping a mounted file
walks each object as it is read:

```rust
let mut objects = stream.objects();
while let Some(mut object) = objects.next()? {
    object.read()?.walk(&mut visitor);
}
```

One owned `BinObject` is alive at a time, and the byte buffer under `read` is the handle's
reused one (`bin-streaming.md` [section 4.2](bin-streaming.md#s4.2)). What that costs is the
largest object's expansion, which the TOC bounds before anything is decoded
(`bin-streaming.md` [section 4](bin-streaming.md#s4)).

**From merge and diff.** `Bin::merge` walks two trees at once and mutates one; `Bin::diff` walks
two and emits records. Neither is a `Visitor` walk. Both keep a `Trail` as they go and build a
`ValuePath` from it at each position they report - `Replaced::at`, `Lift::at` - so an address
means the same thing whichever operation produced it, and no report allocates for a position it
does not name.

## <a id="s7"></a>7. Testing

Unit tests in `crates/ltk_meta/src/walk/` and `crates/ltk_meta/src/path/`, over a synthetic
tree built with the builders that exercises every row of [section 5.1](#s5.1): a `Struct` and an
`Embedded` at a property, inside a container, inside an optional, as a map value; a null pointer
in each position; a container of strings; a map keyed by every kind `Kind::is_valid_map_key`
admits.

- **Order and count.** A visitor that records `(object_hash, class_hash, hash form)` per node
  matches a hand-written list, pre-order, each node once.
- **Pruning.** A visitor that declines one property sees no node beneath it and every node
  elsewhere; `leaves` is called exactly once per `enters` that returned `true`, in reverse
  nesting order.
- **The trail.** `Trail::to_string()` equals `to_value_path().to_string()` at every node, and a
  walk over a map of 10,000 hash-keyed entries allocates nothing in the trail (a counting
  allocator, or the trail's capacity measured before and after).
- **Rendering.** Every row of [section 4.2](#s4.2), in all three forms; `NamedPath::named` plus
  `unnamed` equals the number of field steps plus hash-kind keys; `Unnameable::step` is the first
  unspellable step, not the last.
- **The round trip.** For every node and every leaf position of the fixture tree, with a complete
  name table, `to_property_path` then `Bin::resolve` lands on the value the walk was at
  (FR-13, AC-7 of PRD-001).
- **`BinOverride::walk`** visits the fixture patch's embedded objects and never a record's value.
- **Corpus, `#[ignore]`, under `LTK_LOL_GAME_DIR`.** Every object in the install walks with a
  counting visitor, and the node count equals the count of `Struct` and `Embedded` values with a
  non-zero class plus one per object, computed by an independent recursion in the test.

## <a id="s8"></a>8. Rules

Every rule too small to hold a section of its own, in one table, ordered by subject. **Rule** is
what the crate does, **Instead of** the alternative weighed and rejected, **Spec** where the
behaviour is specified in full. A row whose Spec names an **ADR** is argued there; the row states
the rule and no more.

`Wn` is a stable citation key. A rule that changes keeps its ID and has its row rewritten; new
rules append.

| ID | Rule | Instead of | Why | Spec |
| -- | ---- | ---------- | --- | ---- |
| W1 | The walk's prune is `holds_node`, built on `Kind::is_node` (`Struct`, `Embedded`). `Kind::is_primitive` plays no part. | Entering everything `is_primitive` does not cover. | `ObjectLink` and `BitBool` are neither primitive nor a node, so the complement of `is_primitive` enters containers that hold nothing; and a consumer should not have to know which set `is_primitive` is. | [section 3](#s3) |
| W2 | A `Struct` or `Embedded` with class 0 is not a node and is not entered. | Visiting it as a node with class 0. | It is the client's null pointer, has no properties, and the resolver already treats it as one (`NullPointer`). A visitor keyed on class would otherwise see a class no meta class dump has. | [section 5.1](#s5.1) |
| W3 | `ltk_meta` owns one single-visitor walk with a trail; scheduling several visitors over one walk, and what each does with a node, is the consumer's. | A multi-visitor walk with per-visitor pruning in the crate; or only a predicate and a step enum. | The single-visitor descent is identical for every consumer and is what merge and diff need; the active-set policy is one consumer's and would pin its shape under semver. | [section 5](#s5); ADR-0013 |
| W4 | A `ValuePath` keeps a class context beside its steps - the class of the node each field was read on - and `Step::Field` carries the field hash alone. | `Field { class, field }`, or no class anywhere. | Naming a field takes the class it is on, and every table a consumer holds is keyed by class; keeping it beside the steps leaves `Step` the address and the context free to grow. | [section 4.1](#s4.1); ADR-0012 |
| W5 | `ValuePath` is `PartialEq` and not `Eq` or `Hash`; the hash form is the key. | Deriving `Eq` and `Hash`, or a key type that excludes floats. | `Kind::is_valid_map_key` admits `F32`, `Vector2` and the rest, so a key can be a float. The hash form is total, stable and `Eq`. | [section 4.1](#s4.1) |
| W6 | The walk is infallible and has no early exit. | `visit` returning `ControlFlow`, or a `Result`. | It walks a tree the caller already holds. A visitor that wants to stop declines every further `enters`; a search that wants the first hit is a follow-on with a consumer. | [section 5](#s5) |
| W7 | A visitor is asked `enters` at properties only; the items of a container, optional or map it entered are all descended. | Asking per item. | An item has no field hash to prune on, and pruning per item would make a container of ten thousand structs ten thousand calls for a decision already taken. | [section 5.1](#s5.1) |
| W8 | `leaves` is paired with every `enters` that returned `true`. | `enters` alone. | A consumer running several visitors over one walk carries an active set down the recursion and needs the point to pop it; without the pair it would reconstruct depth from the trail. | [section 5](#s5) |
| W9 | The trail borrows map keys; `ValuePath` owns them with metadata dropped. Two step types, `TrailStep<'a, M>` and `Step`, each beside a class context. | One owned step type in both places. | Owning a key per push allocates for every string-keyed entry descended; borrowing costs nothing and the copy happens only for a reported node. | [section 5.2](#s5.2) |
| W10 | `Index` is `usize`. | `u32`, the width of the wire count. | It indexes a `Vec` and is compared with `len()`; the wire width is the writer's concern. | [section 4.1](#s4.1) |
| W11 | `to_property_path` writes a `Hash` key as its raw decimal value; `to_named` writes the name where one is known. | Writing the name in both. | The value is what is attested; the client coerces a number and a string alike, and a number cannot be mis-hashed. | [section 4.2](#s4.2) |
| W12 | `BinOverride::walk` walks embedded objects only. | Walking record values too, at their record's path. | A record's value is a fragment with no node to stand on until applied; a consumer that wants applied content applies first. | [section 5](#s5) |
| W13 | `FieldNames::field` returns `Cow<'_, str>`. | `&str`, or `String`. | Matches `ltk_ritobin::HashProvider`, so its provider implements this trait without copying, and a computed name is possible. | [section 4.3](#s4.3) |
| W14 | `ValuePath` and `FieldNames` live in `ltk_meta::path` beside `PropertyPath`; the walk in `ltk_meta::walk`. | Everything under `walk`. | Both paths are addresses and are converted between; the walk is one producer of them. | [section 4](#s4), [section 5](#s5) |
| W15 | A class of 0 in the context means unknown; `Trail` never records one, `FromIterator<Step>` and `push(Step)` always do. | `Vec<Option<BinHash>>`. | No node carries the null class (W2), so 0 is free, and the public reading is `Option` through `fields()` either way. | [section 4.1](#s4.1) |
| W16 | Two paths with the same steps are equal whatever their class context. | Comparing the context too. | The context is what a name table is asked with, not where the position is; a report keyed on an address must match the same position however it was reached. | [section 4.1](#s4.1) |
