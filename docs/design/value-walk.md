# Walking a bin's value tree in `ltk_meta`

The API spec for `ltk_meta::walk` and `ltk_meta::path::ValuePath`: one read-only traversal over
every node of an object, driven by a visitor with a prune, running over the owned tree or the
streaming view alike, and the address type a position in that tree is reported with.

**This document states what is true now.** Where the code and a section here disagree, the section
is the bug and gets edited. Two things it does not hold:

- **Why this exists, who asks for it, and what it must do** -
  `docs/prd/001-ptch-property-patches.md` (FR-7, FR-8, FR-12, FR-13), cited here as FR-N.
- **Why an option was chosen over the alternatives it beat** - ADR-0005, ADR-0012, ADR-0013 and
  ADR-0014, cited from the rules in [section 8](#s8).

Designed and not yet built, tracked as #219 (`ValuePath`) and #225 (the walk).

## <a id="s1"></a>1. Summary

Every consumer that reads a bin for what it contains ends up writing the same recursion: match
the six recursive variants of `PropertyValueEnum`, keep a stack of where you are, render that
stack as text when something is worth reporting. `ltk-manager` has two copies of it today,
`ltk_ritobin` has one in its typechecker, and `Bin::merge` and `Bin::diff`
(`ptch-property-patches.md` [section 10](ptch-property-patches.md#s10) and
[section 12](ptch-property-patches.md#s12)) each need one more. This module owns it once.

The module holds:

- **A tree abstraction**, `TreeNode` and `TreeValue`, sealed and implemented twice: by the owned
  tree (`PropertyValueEnum`) and by the streaming views (`ValueView`). A visitor is written
  against the traits and runs over either ([section 3](#s3)).
- **`ValuePath`**, the address of a position inside one object, by hash and by position, with a
  stable hex rendering and a best-effort named one. It names no value model: a map key is a
  `MapKey` ([section 4](#s4)).
- **The walk**: `BinObject::walk`, `ObjectView::walk` and the file-level entry points over
  `Bin`, `BinOverride` and `BinStream`, which call a `Visitor` once per node in a fixed order,
  ask it before entering each property, and carry a `Trail` that renders to a `ValuePath` only
  when asked ([section 5](#s5)).

Over a stream nothing is materialised: the walk crosses an object's buffered bytes, decodes a
header where it has to descend, and hands the visitor leaves it can read without allocating
(ADR-0014). Everything a consumer builds on top - running several visitors over one walk,
deciding what to report, keeping a budget - stays with the consumer (ADR-0013).

## <a id="s2"></a>2. Vocabulary

Every term this document uses in a specific sense.

**The tree**

- **object** - a top-level `BinObject` or `ObjectView`: a path hash, a class hash and properties.
- **node** - an object, or a nested `Struct` or `Embedded` value whose class hash is not 0. A node
  is the thing a visitor is called on: it has a class and properties. A `Struct` with class 0 is
  the client's null pointer; it has no properties and is not a node.
- **property** - one `(field hash, value)` pair of a node, in the node's property order.
- **leaf** - a value that holds no node and cannot hold one: every primitive, `String`, `Hash`,
  `File`, `Link`, `Flag`, and a container, optional or map whose item kind is not a node kind. A
  `Kind` is a **node kind** when it is `Struct` or `Embedded`. A leaf's decoded value is a
  `Leaf`.
- **tree** - either source of nodes and values: the **owned tree**, `PropertyValueEnum<M>`
  under a `BinObject`; or the **view**, `ValueView<'a, M>` under an `ObjectView` over an object's
  buffered bytes (`bin-streaming.md` [section 4.3](bin-streaming.md#s4.3)). `TreeNode` and
  `TreeValue` are what the walk sees of either.

**Moving through it**

- **walk** - the traversal: for one object, visit the root, then for each property the visitor
  enters, descend the value, visiting every node reached. Pre-order, in file order, exactly once
  per node.
- **visitor** - the consumer's side of the walk: `enter_node` and `exit_node` around every
  node, `enter_property` for every property and `exit_property` after each one descended. Each
  answers a `Visit`.
- **enter** and **prune** - the walk descends a property's value when it can hold a node and
  the visitor did not answer `Skip`, and otherwise leaves it whole. A visitor that skips a value
  is not shown anything beneath it.
- **descend** - the walk crossing a container, optional or map to reach the nodes inside it.
  Descent is never asked about: once a property is entered, every node inside it is visited.

**Addresses**

- **step** - one move from a node toward a position inside it: a field, an index or a map key.
  A step carries no class; the class of each node a path passes through is the path's **class
  context**, kept beside the steps (ADR-0012).
- **trail** - the walk's own record of the steps from the root object to its current position.
  It borrows the tree, allocates nothing per step, and is what a visitor renders an address from.
- **`ValuePath`** - the owned address: the trail's steps, with map keys decoded into `MapKey`.
  Addresses a position *inside one object*; the object's hash is carried beside it, never in it
  (D13 in `ptch-property-patches.md` [section 17](ptch-property-patches.md#s17)).
- **hash form** - a `ValuePath` rendered with every field hash as eight hex digits. Stable across
  machines and name tables, and the form to compare and key on.
- **named form** - the same path with every hash a name table can spell replaced by its name.
  Best effort: what the table cannot spell stays hex, and the result says how much it spelled.
- **name table** - anything implementing `FieldNames`: the plaintext behind a field hash on a
  given class, and behind a hash-kind map key.

## <a id="s3"></a>3. The tree the walk sees

```rust
impl Kind {
    /// Whether a value of this kind is a node: `Struct` or `Embedded`.
    ///
    /// The other question - which kinds a value model treats as leaves - is
    /// [`Kind::is_primitive`], and the two are not complements: `ObjectLink` and `BitBool`
    /// are neither primitive nor a node.
    pub fn is_node(self) -> bool;
}
```

The walk is written once, against two sealed traits, and the owned tree and the view each
implement them. A visitor is generic over the value type and never names either.

```rust
/// A value the walk can cross. Sealed: implemented for `&'a PropertyValueEnum<M>` and for
/// `ValueView<'a, M>`, and by nothing else.
pub trait TreeValue<'a>: Copy + sealed::Sealed {
    /// The node type this tree's `Struct` and `Embedded` values are.
    type Node: TreeNode<'a, Value = Self>;
    /// The values inside a container, optional or map, each with the child step reaching it.
    type Children: Iterator<Item = Result<(Child<Self>, Self), Error>>;

    fn kind(&self) -> Kind;

    /// Whether entering this value can reach a node.
    ///
    /// True for a `Struct` or `Embedded` whose class hash is not 0, and for a container,
    /// optional or map whose item kind [`Kind::is_node`]. An empty optional or container of a
    /// node kind answers true: it *can* hold one, and entering it costs nothing.
    ///
    /// # Errors
    ///
    /// Over a view, a header that does not decode. The owned tree never fails.
    fn holds_node(&self) -> Result<bool, Error>;

    /// This value as a node, if it is a `Struct` or `Embedded` with a class hash that is not 0.
    fn as_node(&self) -> Result<Option<Self::Node>, Error>;

    /// The values inside this one, with the step reaching each. Empty for a leaf and for a
    /// node: a node's contents are its properties.
    fn children(&self) -> Result<Self::Children, Error>;

    /// This value decoded, if it is a leaf kind. `None` for every complex kind.
    fn leaf(&self) -> Result<Option<Leaf<'a>>, Error>;

    /// This value as a map key. Every kind `Kind::is_valid_map_key` admits converts.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidKeyType`] for a kind no map can be keyed by.
    fn map_key(&self) -> Result<MapKey, Error>;

    /// The whole value, owned. Allocates; a visitor reaches for it when it needs a subtree
    /// rather than a leaf.
    fn to_value(&self) -> Result<PropertyValueEnum, Error>;
}

/// A node the walk can visit: a class and properties. Sealed: implemented for the owned
/// tree's node, for `StructView<'a, M>`, and for `ObjectView<'a, M>` as a root.
pub trait TreeNode<'a>: Copy + sealed::Sealed {
    type Value: TreeValue<'a, Node = Self>;
    /// The properties in file order. Items are `Result` because a view's kind byte can fail to
    /// decode; the owned tree never fails.
    type Properties: Iterator<Item = Result<(BinHash, Self::Value), Error>>;

    fn class_hash(&self) -> BinHash;
    fn properties(&self) -> Self::Properties;
    /// One property by field hash: the owned tree's keyed lookup, or the view's in-place
    /// scan (`ObjectView::property`).
    fn property(&self, field: BinHash) -> Result<Option<Self::Value>, Error>;
    /// The whole node, owned, as a `Struct` carrying this class and every property. Allocates;
    /// for a root, the object's path hash is `Node::object_hash`.
    fn to_struct(&self) -> Result<values::Struct, Error>;
}

/// The step from a container, optional or map to one value inside it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Child<V> {
    /// A container element, or the value of a present optional (always 0).
    Index(usize),
    /// A map entry, by its key value.
    Key(V),
}

/// A leaf, decoded and borrowed. The client's names for the tags, not the wire enum's:
/// `File` is `Kind::WadChunkLink`, `Link` is `Kind::ObjectLink`, `Flag` is `Kind::BitBool`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Leaf<'a> {
    None,
    Bool(bool),
    I8(i8), U8(u8), I16(i16), U16(u16), I32(i32), U32(u32), I64(i64), U64(u64),
    F32(f32),
    Vector2(Vec2), Vector3(Vec3), Vector4(Vec4), Matrix44(Mat4),
    Color(Color),
    String(&'a str),
    Hash(BinHash),
    File(WadHash),
    Link(BinHash),
    Flag(bool),
}

impl<'a, M> TreeValue<'a> for &'a PropertyValueEnum<M> { type Node = OwnedNode<'a, M>; /* ... */ }
impl<'a, M: Default> TreeValue<'a> for ValueView<'a, M> { type Node = StructView<'a, M>; /* ... */ }

/// The owned tree's node: a class hash and a borrowed property map. `BinObject` and
/// `values::Struct` both view as one.
#[derive(Clone, Copy, Debug)]
pub struct OwnedNode<'a, M = NoMeta> { /* class_hash, &'a IndexMap<BinHash, PropertyValueEnum<M>> */ }
impl<'a, M> TreeNode<'a> for OwnedNode<'a, M> { type Value = &'a PropertyValueEnum<M>; /* ... */ }
impl<'a, M: Default> TreeNode<'a> for StructView<'a, M> { type Value = ValueView<'a, M>; /* ... */ }
impl<'a, M: Default> TreeNode<'a> for ObjectView<'a, M> { type Value = ValueView<'a, M>; /* ... */ }
```

`PropertyValueEnum::holds_node` is also exposed directly, infallible, and `Kind::is_primitive`
plays no part in any of this (W1). `Leaf` is the one place the crate names the tags as the
client does (W19): a visitor reads a texture path as `Leaf::File`, whatever `Kind` calls it.

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
/// when the file would write the same bytes for them, which is the only equality a map on the
/// wire has. The client's names for the tags, as `Leaf` (W19).
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

What that round trip attests is this crate's resolver, not the client's. The `{key}` literal is
JSON by D10, inferred from `PropertyPath.hpp`; the reversing notes' worked example writes the
bare text `PerAttachmentMaterial{weapon}`, no shipped record uses a `{key}` at all, and the two
readings have not been settled in game. A path with a `Key` step is therefore **unattested** as
a client path, and a tool that exports one should say so until D10 is tested (W18). Field-only
paths and `[i]` subscripts are the forms every shipped record uses and carry no such caveat.

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

The class in the context is the **concrete** class, as the file states it: the object's class
hash, or the class hash a `Struct` or `Embedded` carries. For a pointer that is the class the
client constructs, which may be a descendant of the class the property declares
(`BinPropertyTypes_TypeRule.md` section 9.5). Two things follow for a class-keyed table:

- A field may be declared on a base class rather than on the concrete one. The client resolves a
  field by walking the base chain from the concrete class (`MetaPath_resolve`, `cls+56`), and a
  table keyed by class has to do the same; `field(field, Some(class))` is asked with the concrete
  class only, and walking the chain is the table's job (W17).
- The concrete class is what a meta class dump names a field under, which is why it is the
  right one to carry. The declared class of the property is not recorded anywhere on the path.

## <a id="s5"></a>5. The walk

The visitor has the shape of `ltk_ritobin`'s CST visitor (`cst::visitor`): symmetric enter and
exit callbacks, each answering a `Visit` that continues, skips, stops or aborts, and a walk that
reports how it ended (W21). Here there are two nested pairs, because a property is not a leaf:
a node is entered and exited, and so is each property that can hold a node.

```rust
/// What a callback answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Visit {
    /// Ends the walk immediately. No exit callback runs for anything still open.
    Abort,
    /// Ends the walk after unwinding: every open property and node still gets its exit,
    /// innermost first. The walk does not resume.
    Stop,
    /// Skips ahead, locally:
    /// - from `enter_node`: the node's properties are not walked; its `exit_node` still runs.
    /// - from `enter_property`: the value is not descended - the prune. `exit_property` still
    ///   runs.
    /// - from `exit_property`: the node's remaining properties are pruned; the walk jumps to
    ///   the node's `exit_node`.
    /// - from `exit_node`: the parent property's remaining items are pruned; the walk jumps to
    ///   the parent's `exit_property`.
    Skip,
    Continue,
}

/// How a walk ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkOutcome {
    /// Every object was walked to the end.
    Completed,
    /// A callback answered `Visit::Stop` and the walk unwound.
    Stopped,
    /// A callback answered `Visit::Abort`.
    Aborted,
}

/// What a walk calls. Generic over the tree's value type, so one visitor runs over the owned
/// tree (`V = &PropertyValueEnum<M>`) and over the view (`V = ValueView<'a, M>`) alike.
///
/// Every callback has a default that continues, so a visitor implements only what it reads.
pub trait Visitor<'a, V: TreeValue<'a>> {
    /// The visitor's own error. The tree's errors convert into it, so a `?` on a tree call
    /// inside a callback needs nothing more than `From<ltk_meta::Error>`.
    type Error: From<Error>;

    /// Called at every node the walk reaches, before any of its properties.
    fn enter_node(&mut self, node: &Node<'_, 'a, V>) -> Result<Visit, Self::Error> {
        Ok(Visit::Continue)
    }
    /// Called once for every node entered - after its properties, after a `Skip`, and while
    /// unwinding for a `Stop`. Never after an `Abort`.
    fn exit_node(&mut self, node: &Node<'_, 'a, V>) -> Result<Visit, Self::Error> {
        Ok(Visit::Continue)
    }
    /// Called for every property of a node, in file order, leaves included - the value is the
    /// tree's, undecoded until read. Only a value that `holds_node` is descended on
    /// `Continue`; a leaf is a call and nothing more.
    fn enter_property(&mut self, field: BinHash, value: V, node: &Node<'_, 'a, V>)
        -> Result<Visit, Self::Error> {
        Ok(Visit::Continue)
    }
    /// Called once for every property that was descended - after its nodes, after a `Skip`,
    /// and while unwinding for a `Stop`. Not called for a leaf. Never after an `Abort`.
    fn exit_property(&mut self, field: BinHash, value: V, node: &Node<'_, 'a, V>)
        -> Result<Visit, Self::Error> {
        Ok(Visit::Continue)
    }
}

/// A `&mut W` is a visitor, so a `&mut dyn Visitor<'a, V, Error = E>` can be passed where one
/// is wanted.
impl<'a, V: TreeValue<'a>, W: Visitor<'a, V> + ?Sized> Visitor<'a, V> for &mut W {}

/// One node, as the walk hands it to a visitor.
#[derive(Clone, Copy, Debug)]
pub struct Node<'t, 'a, V: TreeValue<'a>> { /* object hash, V::Node, &'t Trail<V> */ }

impl<'t, 'a, V: TreeValue<'a>> Node<'t, 'a, V> {
    /// The path hash of the object this node is in, or is.
    pub fn object_hash(&self) -> BinHash;
    /// The class hash this node carries. Never 0.
    pub fn class_hash(&self) -> BinHash;
    /// The node itself: its properties, in file order, lookup by field, and `to_struct`.
    pub fn inner(&self) -> V::Node;
    /// Where the node is: empty at the root.
    pub fn trail(&self) -> &'t Trail<V>;
    /// Whether this node is the object itself.
    pub fn is_root(&self) -> bool;
    /// The node's address, copied out of the trail. Allocates; call it for a node worth
    /// reporting on.
    pub fn value_path(&self) -> Result<ValuePath, Error>;
}

/// The steps from an object's root to the walk's position.
///
/// Borrows the tree - a map key is the tree's own value, never a copy - so descending a map of
/// ten thousand entries allocates nothing. Text is made only by `to_value_path` or `Display`.
#[derive(Debug)]
pub struct Trail<V> { /* Vec<TrailStep<V>>, Vec<BinHash> */ }

/// One step of a [`Trail`]. The borrowing form of [`Step`]: a key is the tree's value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrailStep<V> {
    Field(BinHash),
    Index(usize),
    Key(V),
}

impl<'a, V: TreeValue<'a>> Trail<V> {
    pub fn steps(&self) -> &[TrailStep<V>];
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    /// The class of the node each field step was read on, one per `Field` step, in order.
    /// Never 0: the walk always knows.
    pub fn classes(&self) -> &[BinHash];
    /// The owned address: every step copied, every key decoded to a `MapKey`, the class
    /// context carried over.
    ///
    /// # Errors
    ///
    /// Over a view, a key that does not decode. The owned tree never fails.
    pub fn to_value_path(&self) -> Result<ValuePath, Error>;
}

/// The hash form of section 4.2, without building a `ValuePath`. A key that does not decode
/// renders as `{?}`.
impl<'a, V: TreeValue<'a>> fmt::Display for Trail<V> {}

/// The owned tree. Never fails unless the visitor does.
impl<M> BinObject<M> {
    /// Walks this object: the root, then every node beneath every property `visitor` enters.
    pub fn walk<'a, W>(&'a self, visitor: &mut W) -> Result<WalkOutcome, W::Error>
    where W: Visitor<'a, &'a PropertyValueEnum<M>>;
}
impl<M> Bin<M> {
    /// Walks every object, in file order. A `Stop` or `Abort` ends the whole walk, not the
    /// current object.
    pub fn walk<'a, W>(&'a self, visitor: &mut W) -> Result<WalkOutcome, W::Error>
    where W: Visitor<'a, &'a PropertyValueEnum<M>>;
}
impl<M> BinOverride<M> {
    /// Walks every embedded object, in file order, as the file holds them: a record that
    /// targets one of them (the client merges a patch's objects into the base table before
    /// applying records, so it may) has not been applied. Patch records are not walked: a
    /// record's value has no node of its own to stand on.
    pub fn walk<'a, W>(&'a self, visitor: &mut W) -> Result<WalkOutcome, W::Error>
    where W: Visitor<'a, &'a PropertyValueEnum<M>>;
}

/// The view.
impl<'a, M: Default> ObjectView<'a, M> {
    /// Walks this object over its buffered bytes: nothing is materialised, a header is
    /// decoded where the walk descends, and a leaf is decoded only when the visitor asks.
    ///
    /// # Errors
    ///
    /// A kind byte or header that does not decode, converted into the visitor's error, or
    /// whatever the visitor raises.
    pub fn walk<W>(&self, visitor: &mut W) -> Result<WalkOutcome, W::Error>
    where W: Visitor<'a, ValueView<'a, M>>;
}
impl<R: io::Read + io::Seek, M: Default> ObjectStream<'_, R, M> {
    /// `view()?` then `walk`.
    pub fn walk<W>(&mut self, visitor: &mut W) -> Result<WalkOutcome, W::Error>
    where W: for<'a> Visitor<'a, ValueView<'a, M>>;
}
impl<R: io::Read + io::Seek, M: Default> BinStream<R, M> {
    /// Walks every object in file order, one buffered object at a time: `objects()` and
    /// `walk` on each. Holds one object's bytes at any moment and nothing of the tree.
    pub fn walk<W>(&mut self, visitor: &mut W) -> Result<WalkOutcome, W::Error>
    where W: for<'a> Visitor<'a, ValueView<'a, M>>;
}
```

A visitor written against `TreeValue` and `TreeNode` runs over either tree unchanged. The
manager's rules are written once and run over `BinStream::walk` in the pass and over
`BinObject::walk` when a repair verifies the tree it just edited in memory
([section 6](#s6)). The `for<'a>` bound on the stream entry points is what lets one visitor
value outlive every object buffer it is shown.

The walk stops at the first error and returns it, exits and all skipped, as for an `Abort`.
Over the owned tree only a visitor can raise one; over a view, a header that does not decode
does too, which the corpus says never happens for a shipped file (`bin-streaming.md`
[appendix A](bin-streaming.md#appendix-a)). An early exit that is not an error is `Visit::Stop`
or `Visit::Abort`, and the outcome says which (W6).

**Collapsing a subtree eagerly.** A visitor that wants a property in full rather than a walk
over it reads it and prunes it in one place:

```rust
fn enter_property(&mut self, field: BinHash, value: V, node: &Node<'_, 'a, V>)
    -> Result<Visit, Self::Error> {
    if node.class_hash() == BANK_UNIT && field == BANK_PATH {
        self.banks.push(value.to_value()?);   // the whole list, one decode
        return Ok(Visit::Skip);               // and nothing beneath it is walked
    }
    Ok(Visit::Continue)
}
```

The same at a node boundary is `enter_node` calling `node.inner().to_struct()?` and answering
`Skip`. Over a view either is one decode of exactly that sized region; over the owned tree it
is a clone.

### <a id="s5.1"></a>5.1 Traversal rules

For one object:

1. The object is a node with an empty trail. `enter_node` is called on it.
2. For each property `(field, value)` of a node, in property order, the walk calls
   `enter_property(field, value, node)`. Then it asks the tree `holds_node(value)`: if not,
   the value is a leaf or holds only leaves and is left whole - over a view, skipped by its
   declared size, decoding nothing - and no `exit_property` follows. If it does hold a node and
   the visitor answered `Skip`, the value is left whole the same way and `exit_property` runs.
3. Otherwise `Field(field)` is pushed, with the node's class hash recorded in the class
   context, and the walk descends `value`:
   - `Struct` or `Embedded` with class hash not 0: a node. `enter_node`, then rule 2 recurses,
     then `exit_node`.
   - `Struct` or `Embedded` with class hash 0: a null pointer. Nothing.
   - `Container` or `UnorderedContainer`: for each item, `Index(i)` is pushed, the item is
     descended, and the step is popped.
   - `Optional` holding a value: `Index(0)` is pushed, the value is descended, the step is
     popped. An optional is indexed rather than stepped through, because that is how the path
     grammar addresses it (D9 in `ptch-property-patches.md`).
   - `Map`: for each entry, `Key(key)` is pushed, the value is descended, the step is popped.
     Keys are never descended; a map key is a leaf by construction.
   - Anything else holds no node. Nothing.
4. The `Field` step is popped and `exit_property(field, value, node)` is called.
5. `exit_node` is called on the node.

`Visit::Skip` from `enter_node` runs rule 5 without rule 2; from `exit_property` it runs rule 5
without the node's remaining properties; from `exit_node` it ends the enclosing property's
remaining items and runs rule 4. `Stop` runs every pending rule 4 and rule 5 innermost first,
then returns `Stopped`; `Abort` returns `Aborted` at once.

So a visitor sees nodes in pre-order, in file order, each exactly once, and every push is popped
before the walk returns. With every callback at its default the walk visits every node in the
object; a visitor is shown every property, leaves included, and is never asked about the items
inside a container, optional or map it chose to descend.

The file-level entry points apply the above to each object in file order, with the trail emptied
between objects. Nothing carries over from one object to the next.

### <a id="s5.2"></a>5.2 The trail

The trail holds hashes, indices and the tree's own key values, never text. A step costs a push.
`Display` on a `Trail` writes the hash form straight from the tree, and `to_value_path` copies
the steps out decoding each key to a `MapKey`; either is what a visitor does for a node it
reports on, and neither happens otherwise.

Beside the steps the trail keeps the class context: for each `Field` step, the class hash of
the node the field was read on - the object's class hash at the root, the `Struct` or
`Embedded` class hash below it. It is what a name table is asked with ([section 4.3](#s4.3)),
and `to_value_path` carries it over.

## <a id="s6"></a>6. Where else the tree and the trail are used

**From the stream.** `BinStream::walk` is the pass: one buffered object at a time, no
`BinObject` anywhere. What it costs is the file's bytes plus the largest object's buffer, which
the TOC bounds before anything is decoded (`bin-streaming.md`
[section 4](bin-streaming.md#s4)). The eager `read()` path stays for a consumer that wants to
keep the object, and the same visitor runs over it.

**From a repair.** A consumer that edits an owned tree and wants to check its work runs the
same visitor over `BinObject::walk`. That is `ltk-manager`'s `bin_property_type::fix`
verifying in memory; the check and its verification are one visitor.

**From merge and diff.** `Bin::merge` walks two owned trees at once and mutates one;
`Bin::diff` walks two and emits records. Neither is a `Visitor` walk. Both keep a `Trail` as
they go and build a `ValuePath` from it at each position they report - `Replaced::at`,
`Lift::at` - so an address means the same thing whichever operation produced it, and no report
allocates for a position it does not name.

## <a id="s7"></a>7. Testing

Unit tests in `crates/ltk_meta/src/walk/` and `crates/ltk_meta/src/path/`, over a synthetic
tree built with the builders that exercises every row of [section 5.1](#s5.1): a `Struct` and an
`Embedded` at a property, inside a container, inside an optional, as a map value; a null pointer
in each position; a container of strings; a map keyed by every kind `Kind::is_valid_map_key`
admits. The tree is written to bytes once so every test below runs twice, over the owned tree
and over an `ObjectView` of the same bytes, through one generic visitor.

- **Order and count.** A visitor that records `(object_hash, class_hash, hash form)` per node
  matches a hand-written list, pre-order, each node once, and the two trees produce the same
  list.
- **Pruning and flow.** A visitor that answers `Skip` at one property sees no node beneath it
  and every node elsewhere; `exit_property` runs exactly once per property descended and never
  for a leaf, `exit_node` exactly once per node entered, in reverse nesting order; `Skip` from
  each of the four callbacks prunes exactly what [section 5.1](#s5.1) says; `Stop` unwinds every
  open exit and returns `Stopped`; `Abort` runs no further callback and returns `Aborted`; a
  visitor error ends the walk like an `Abort` and is returned.
- **The trail.** `Trail::to_string()` equals `to_value_path()?.to_string()` at every node, and a
  walk over a map of 10,000 hash-keyed entries allocates nothing in the trail (a counting
  allocator, or the trail's capacity measured before and after). Over a view, nothing beyond
  the trail allocates at all.
- **Leaves and keys.** `leaf()` over both trees agrees for every leaf kind; `map_key()` agrees
  and round-trips through `MapKey::to_value`; two `F32` keys with the same bits are equal and
  `Hash` on them agrees.
- **Rendering.** Every row of [section 4.2](#s4.2), in all three forms; `NamedPath::named` plus
  `unnamed` equals the number of field steps plus hash-kind keys; `Unnameable::step` is the first
  unspellable step, not the last.
- **The round trip.** For every node and every leaf position of the fixture tree, with a complete
  name table, `to_property_path` then `Bin::resolve` lands on the value the walk was at
  (FR-13, AC-7 of PRD-001).
- **`BinOverride::walk`** visits the fixture patch's embedded objects and never a record's value.
- **Corpus, `#[ignore]`, under `LTK_LOL_GAME_DIR`.** Every object in the install walks through
  `BinStream::walk` and through `Bin::walk` of the same chunk with a counting visitor; the two
  visit sequences are identical, and the node count equals the count of `Struct` and `Embedded`
  values with a non-zero class plus one per object, computed by an independent recursion in the
  test.

## <a id="s8"></a>8. Rules

Every rule too small to hold a section of its own, in one table, ordered by subject. **Rule** is
what the crate does, **Instead of** the alternative weighed and rejected, **Spec** where the
behaviour is specified in full. A row whose Spec names an **ADR** is argued there; the row states
the rule and no more.

`Wn` is a stable citation key. A rule that changes keeps its ID and has its row rewritten; new
rules append.

| ID | Rule | Instead of | Why | Spec |
| -- | ---- | ---------- | --- | ---- |
| W1 | The walk's prune is `holds_node`, built on `Kind::is_node` (`Struct`, `Embedded`), asked of the tree before the visitor. `Kind::is_primitive` plays no part. | Entering everything `is_primitive` does not cover. | `ObjectLink` and `BitBool` are neither primitive nor a node, so the complement of `is_primitive` enters containers that hold nothing; and a consumer should not have to know which set `is_primitive` is. | [section 3](#s3), [section 5.1](#s5.1) |
| W2 | A `Struct` or `Embedded` with class 0 is not a node and is not entered. | Visiting it as a node with class 0. | It is the client's null pointer, has no properties, and the resolver already treats it as one (`NullPointer`). A visitor keyed on class would otherwise see a class no meta class dump has. | [section 5.1](#s5.1) |
| W3 | `ltk_meta` owns one single-visitor walk with a trail; scheduling several visitors over one walk, and what each does with a node, is the consumer's. | A multi-visitor walk with per-visitor pruning in the crate; or only a predicate and a step enum. | The single-visitor descent is identical for every consumer and is what merge and diff need; the active-set policy is one consumer's and would pin its shape under semver. | [section 5](#s5); ADR-0013 |
| W4 | A `ValuePath` keeps a class context beside its steps - the class of the node each field was read on - and `Step::Field` carries the field hash alone. | `Field { class, field }`, or no class anywhere. | Naming a field takes the class it is on, and every table a consumer holds is keyed by class; keeping it beside the steps leaves `Step` the address and the context free to grow. | [section 4.1](#s4.1); ADR-0012 |
| W5 | A map key in a `ValuePath` is a `MapKey`, owned and metadata-free, with floats held as bits; `ValuePath` is `Eq` and `Hash`. | `Key(PropertyValueEnum)`, and `PartialEq` only. | The address type must not name the value model, and a repair keys its findings on the address. Bit equality is the only equality a map on the wire has. | [section 4.1](#s4.1) |
| W6 | Every callback answers `Result<Visit, Visitor::Error>`; the walk returns `Result<WalkOutcome, Visitor::Error>`, with the tree's errors converted through `From`. | An infallible walk with a `bool` prune; or `ltk_meta::Error` as the only error. | Over a view a header can fail to decode and a visitor reading a leaf can too, so the walk is fallible; the error is the visitor's because the visitor is what the caller wrote, and a `Stop` is not an error. | [section 5](#s5) |
| W7 | `enter_property` is called for every property, leaves included; a value is descended only when it `holds_node`; the items of a container, optional or map are all descended, never asked about. | Asking only for values that hold a node, or asking per item. | A visitor reads a `File` leaf where it sits instead of iterating the node's properties again; an item has no field hash to prune on, and asking per item would make a container of ten thousand structs ten thousand calls for a decision already taken. | [section 5.1](#s5.1) |
| W8 | `exit_property` is paired with every property descended and `exit_node` with every node entered, including while unwinding for a `Stop`. | Enter callbacks alone. | A consumer running several visitors over one walk carries an active set down the recursion and needs the point to pop it; symmetric exits are also what `ltk_ritobin`'s visitor guarantees. | [section 5](#s5) |
| W9 | The trail holds the tree's own key values; `ValuePath` owns decoded `MapKey`s. `TrailStep<V>` is generic over the tree's value, `Step` is not. | One owned step type in both places. | Owning a key per push allocates for every string-keyed entry descended; holding the tree's value costs nothing and the decode happens only for a reported node. | [section 5.2](#s5.2) |
| W10 | `Index` is `usize`. | `u32`, the width of the wire count. | It indexes a `Vec` and is compared with `len()`; the wire width is the writer's concern. | [section 4.1](#s4.1) |
| W11 | `to_property_path` writes a `Hash` key as its raw decimal value; `to_named` writes the name where one is known. | Writing the name in both. | The value is what is attested; the client coerces a number and a string alike, and a number cannot be mis-hashed. | [section 4.2](#s4.2) |
| W12 | `BinOverride::walk` walks embedded objects only. | Walking record values too, at their record's path. | A record's value is a fragment with no node to stand on until applied; a consumer that wants applied content applies first. | [section 5](#s5) |
| W13 | `FieldNames::field` returns `Cow<'_, str>`. | `&str`, or `String`. | Matches `ltk_ritobin::HashProvider`, so its provider implements this trait without copying, and a computed name is possible. | [section 4.3](#s4.3) |
| W14 | `ValuePath`, `MapKey` and `FieldNames` live in `ltk_meta::path` beside `PropertyPath`; the tree traits, `Leaf` and the walk in `ltk_meta::walk`. | Everything under `walk`. | Both paths are addresses and are converted between; the walk is one producer of them. | [section 4](#s4), [section 5](#s5) |
| W15 | A class of 0 in the context means unknown; `Trail` never records one, `FromIterator<Step>` and `push(Step)` always do. | `Vec<Option<BinHash>>`. | No node carries the null class (W2), so 0 is free, and the public reading is `Option` through `fields()` either way. | [section 4.1](#s4.1) |
| W16 | Two paths with the same steps are equal, and hash the same, whatever their class context. | Comparing the context too. | The context is what a name table is asked with, not where the position is; a report keyed on an address must match the same position however it was reached. | [section 4.1](#s4.1) |
| W17 | The class context holds the concrete class the file states; a class-keyed `FieldNames` walks the base chain itself. | Recording the declaring class, or walking the chain in `to_named`. | The crate holds no schema (ADR-0006), so it cannot know where a field is declared; the client resolves from the concrete class up, and a dump names fields under the class that declares them. | [section 4.3](#s4.3) |
| W18 | A `PropertyPath` produced from a `Key` step is unattested as a client path until D10 is tested in game. | Refusing to produce one. | The resolver's own reading is consistent and round-trips here; what is unknown is whether the client reads the literal as JSON or as bare text, and no shipped record decides it. | [section 4.2](#s4.2) |
| W19 | `Leaf` and `MapKey` name the tags as the client does: `File`, `Link`, `Flag`. `Kind` keeps `WadChunkLink`, `ObjectLink`, `BitBool`. | Reusing `Kind`'s names in the new types. | The new surface is what a consumer writes against and should carry the vocabulary the reversing notes and the meta class dumps use; renaming `Kind` is a break for every existing caller and is its own decision. | [section 3](#s3) |
| W20 | The walk runs over two sealed traits, `TreeNode` and `TreeValue`, implemented by the owned tree and by the views; a visitor is generic over the value type. | A walk over `PropertyValueEnum` only, with `read()` per streamed object; or a walk over the views only. | One traversal, one visitor, both sources; the stream pass materialises nothing and the repair's in-memory check uses the same rule. Sealed, because a third tree would have to be this crate's. | [section 3](#s3), [section 5](#s5); ADR-0014 |
| W21 | The visitor has `ltk_ritobin`'s CST visitor shape: symmetric enter and exit, a `Visit` answer of `Abort`, `Stop`, `Skip` or `Continue`, a `WalkOutcome`. `Skip` from `enter_property` prunes that value, where the CST's token `Skip` prunes the rest of the node. | A `bool` prune and no early exit. | One visitor idiom across the workspace; and a property, unlike a token, has a subtree of its own to prune. | [section 5](#s5) |
