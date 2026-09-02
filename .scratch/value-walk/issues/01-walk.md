---
issue: 225
title: "Value walk: one read-only traversal over every node of a bin"
labels: crate:ltk_meta, enhancement, format:bin, area:api, blocked
---

Design: `docs/design/value-walk.md` [section 3](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/value-walk.md#s3) and [section 5](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/value-walk.md#s5); requirements PRD-001 FR-12 and FR-13.
A `Visitor` called once per node of an object, in pre-order and file order, with a prune per
property and a borrowing `Trail` that renders to a `ValuePath` (#219) only for a node a visitor
reports on. The walk is written against two sealed tree traits that the owned tree and the
streaming views both implement, so one visitor runs over a `BinObject` and over a `BinStream`
sweep alike, and the sweep materialises nothing.

The consumer is `ltk-manager`'s problems pass, which reads every bin once and runs every
health-check rule over that one read. It has two hand-written walkers today, each matching the
six recursive `PropertyValueEnum` variants by hand, and a third on the way; with this they are
visitors over `BinStream::walk`, and the same visitor verifies a repair over `BinObject::walk`.
`Bin::merge` (#220) and `Bin::diff` build their addresses from the same `Trail`.

## Proposed surface

In `ltk_meta::walk`:

```rust
/// The one question the walk asks of a `Kind`. Sealed: implemented for `Kind` only.
pub trait TreeKind: Copy + sealed::Sealed {
    /// Whether a value of this kind is a node: `Struct` or `Embedded`.
    fn is_node(self) -> bool;
}
impl TreeKind for Kind {}

/// A value the walk can cross. Sealed: implemented for `&'a PropertyValueEnum<M>` and for
/// `ValueView<'a, M>`, and by nothing else.
pub trait TreeValue<'a>: Copy + sealed::Sealed {
    type Node: TreeNode<'a, Value = Self>;
    type Children: Iterator<Item = Result<(Child<Self>, Self), Error>>;

    fn kind(&self) -> Kind;
    /// Whether entering this value can reach a node: a `Struct` or `Embedded` whose class hash
    /// is not 0, or a container, optional or map whose item kind [`TreeKind::is_node`].
    fn holds_node(&self) -> Result<bool, Error>;
    /// This value as a node, if it is a `Struct` or `Embedded` with a class hash that is not 0.
    fn as_node(&self) -> Result<Option<Self::Node>, Error>;
    /// The values inside this one, with the step reaching each. Empty for a leaf and a node.
    fn children(&self) -> Result<Self::Children, Error>;
    /// This value decoded, if it is a leaf kind.
    fn leaf(&self) -> Result<Option<Leaf<'a>>, Error>;
    /// This value as a map key; `InvalidKeyType` for a kind no map can be keyed by.
    fn map_key(&self) -> Result<MapKey, Error>;
    /// The whole value, owned. Allocates.
    fn to_value(&self) -> Result<PropertyValueEnum, Error>;
}

/// A node the walk can visit: a class and properties. Sealed: implemented for the owned
/// tree's node and for `StructView<'a, M>`. An object's root is walked as a `StructView`
/// over the same bytes.
pub trait TreeNode<'a>: Copy + sealed::Sealed {
    type Value: TreeValue<'a, Node = Self>;
    type Properties: Iterator<Item = Result<(BinHash, Self::Value), Error>>;

    fn class_hash(&self) -> BinHash;
    fn properties(&self) -> Self::Properties;
    fn property(&self, field: BinHash) -> Result<Option<Self::Value>, Error>;
    /// The whole node, owned, as a `Struct`. Allocates.
    fn to_struct(&self) -> Result<values::Struct, Error>;
}

/// The step from a container, optional or map to one value inside it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Child<V> {
    Index(usize),
    Key(V),
}

/// A leaf, decoded and borrowed. The client's names for the tags: `File` is
/// `Kind::WadChunkLink`, `Link` is `Kind::ObjectLink`, `Flag` is `Kind::BitBool`.
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

/// The owned tree's node: a class hash and a borrowed property map. `BinObject` and
/// `values::Struct` both view as one.
#[derive(Clone, Copy, Debug)]
pub struct OwnedNode<'a, M = NoMeta> { /* ... */ }

impl<'a, M> TreeValue<'a> for &'a PropertyValueEnum<M> { type Node = OwnedNode<'a, M>; /* ... */ }
impl<'a, M: Default> TreeValue<'a> for ValueView<'a, M> { type Node = StructView<'a, M>; /* ... */ }
impl<'a, M> TreeNode<'a> for OwnedNode<'a, M> { type Value = &'a PropertyValueEnum<M>; /* ... */ }
impl<'a, M: Default> TreeNode<'a> for StructView<'a, M> { type Value = ValueView<'a, M>; /* ... */ }

/// What a callback answers. `ltk_ritobin::cst::visitor::Visit`'s shape (W21).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Visit {
    /// Ends the walk immediately. No exit callback runs for anything still open.
    Abort,
    /// Ends the walk after unwinding: every open property and node still gets its exit.
    Stop,
    /// Skips ahead, locally: from `enter_node`, the node's properties; from `enter_property`,
    /// the value (the prune); from `exit_property`, the node's remaining properties; from
    /// `exit_node`, the parent property's remaining items.
    Skip,
    Continue,
}

/// How a walk ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkOutcome { Completed, Stopped, Aborted }

/// What a walk calls. Generic over the tree's value type. Every callback defaults to
/// `Continue`.
pub trait Visitor<'a, V: TreeValue<'a>> {
    /// The visitor's own error; the tree's errors convert into it.
    type Error: From<Error>;

    /// Before any of the node's properties.
    fn enter_node(&mut self, node: &Node<'_, 'a, V>) -> Result<Visit, Self::Error> { Ok(Visit::Continue) }
    /// Once per node entered - after its properties, after a `Skip`, while unwinding for a
    /// `Stop`. Never after an `Abort`.
    fn exit_node(&mut self, node: &Node<'_, 'a, V>) -> Result<Visit, Self::Error> { Ok(Visit::Continue) }
    /// For every property, in file order, leaves included. Only a value that `holds_node` is
    /// descended on `Continue`.
    fn enter_property(&mut self, field: BinHash, value: V, node: &Node<'_, 'a, V>)
        -> Result<Visit, Self::Error> { Ok(Visit::Continue) }
    /// Once per property that holds a node and was entered. Not called for a leaf. Never
    /// after an `Abort`.
    fn exit_property(&mut self, field: BinHash, value: V, node: &Node<'_, 'a, V>)
        -> Result<Visit, Self::Error> { Ok(Visit::Continue) }
}

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
    pub fn is_root(&self) -> bool;
    /// The node's address, copied out of the trail. Allocates.
    pub fn value_path(&self) -> Result<ValuePath, Error>;
}

/// The steps from an object's root to the walk's position. A map key is the tree's own
/// value, never a copy.
#[derive(Debug)]
pub struct Trail<V> { /* Vec<TrailStep<V>>, Vec<BinHash> */ }

/// One step of a [`Trail`]. The borrowing form of [`Step`].
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
    /// The class of the node each field step was read on, one per `Field` step. Never 0.
    pub fn classes(&self) -> &[BinHash];
    /// The owned address: every step copied, every key decoded to a `MapKey`, the class
    /// context carried over.
    pub fn to_value_path(&self) -> Result<ValuePath, Error>;
}

/// The hash form, without building a `ValuePath`. A key that does not decode renders `{?}`.
impl<'a, V: TreeValue<'a>> fmt::Display for Trail<V> {}

impl<M> BinObject<M> {
    pub fn walk<'a, W>(&'a self, visitor: &mut W) -> Result<WalkOutcome, W::Error>
    where W: Visitor<'a, &'a PropertyValueEnum<M>>;
}
impl<M> Bin<M> {
    /// Walks every object, in file order.
    pub fn walk<'a, W>(&'a self, visitor: &mut W) -> Result<WalkOutcome, W::Error>
    where W: Visitor<'a, &'a PropertyValueEnum<M>>;
}
impl<M> BinOverride<M> {
    /// Walks every embedded object, in file order, as the file holds them: a record that
    /// targets one of them has not been applied. Patch records are not walked.
    pub fn walk<'a, W>(&'a self, visitor: &mut W) -> Result<WalkOutcome, W::Error>
    where W: Visitor<'a, &'a PropertyValueEnum<M>>;
}

impl<'a, M: Default> ObjectView<'a, M> {
    /// Walks this object over its buffered bytes: nothing is materialised, a header is
    /// decoded where the walk descends, and a leaf is decoded only when the visitor asks.
    pub fn walk<W>(&self, visitor: &mut W) -> Result<WalkOutcome, W::Error>
    where W: Visitor<'a, ValueView<'a, M>>;
}
impl<R: io::Read + io::Seek, M: Default> ObjectStream<'_, R, M> {
    /// `view()?` then `walk`.
    pub fn walk<E, W>(&mut self, visitor: &mut W) -> Result<WalkOutcome, E>
    where E: From<Error>, W: for<'a> Visitor<'a, ValueView<'a, M>, Error = E>;
}
impl<R: io::Read + io::Seek, M: Default> BinStream<R, M> {
    /// Walks every object in file order, one buffered object at a time. Holds one object's
    /// bytes at any moment and nothing of the tree.
    pub fn walk<E, W>(&mut self, visitor: &mut W) -> Result<WalkOutcome, E>
    where E: From<Error>, W: for<'a> Visitor<'a, ValueView<'a, M>, Error = E>;
}
```

## Rationale

**W20 (ADR-0014): two sealed traits, both trees.** The views exist so a consumer pays for what
it reads; a pass that decoded every object to visit it would pay for everything. One visitor,
generic over the value type, runs over `BinStream::walk` in the pass and over `BinObject::walk`
when a repair verifies its own edit in memory. `Leaf` and `MapKey` carry the client's tag names
(W19) and the visitor never names `PropertyValueEnum`, `M` or `ValueView`.

**W21: the visitor is `ltk_ritobin`'s CST visitor shape.** Symmetric enter and exit callbacks,
a `Visit` answer, a `WalkOutcome`; two nested pairs here because a property, unlike a token, has
a subtree to prune. Collapsing a subtree eagerly is `value.to_value()?` or
`node.inner().to_struct()?` in the enter callback followed by `Visit::Skip`.

**W3 (ADR-0013): one visitor, the fan-out stays with the consumer.** The descent with a trail is
the same for every consumer and is what merge and diff need; which visitors are active beneath a
value is one consumer's policy. The exit callbacks are on the trait so that fan-out can be one
`Visitor` that pushes its active set on `enter_property` and pops it on `exit_property` (W8).

**Traversal** is `value-walk.md` [section 5.1](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/value-walk.md#s5.1): pre-order, file order, each node once. The tree is
shown every property through `enter_property`, leaves included, and descends only a value
that `holds_node` (W7); over a view a leaf or a container of leaves is skipped by declared size
with nothing decoded. A `Struct` or `Embedded` with class 0 is the client's null pointer and is not a node
(W2). An optional's value is `Index(0)`, as the path grammar addresses it (D9).

**The trail allocates nothing per step.** Keys are the tree's own values; text and owned steps
are made only by `Display`, `to_value_path` or `Node::value_path`, which a visitor calls for a
node it reports on (W9).

**The walk is fallible over both trees, in the visitor's error (W6).** A view's header can fail
to decode and a visitor reading a leaf can too; the tree's errors convert through `From`, and a
`Stop` is an outcome, not an error.

Blocked by #219: `Node::value_path`, `Trail::to_value_path` and `MapKey` are its types.
`TreeKind`, the tree traits, `Leaf`, `Visitor`, `Node`, `Trail` and every entry point do not
depend on it and can land first behind those methods.

- [ ] The fixture tree of `value-walk.md` [section 7](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/value-walk.md#s7) is walked twice, owned and as an `ObjectView` of its bytes, through one generic visitor, and a
      visitor recording `(object_hash, class_hash, hash form)` per node produces the same
      hand-written pre-order list from both, each node once
- [ ] A `Struct` or `Embedded` with class 0 is never visited, at a property, in a container, in
      an optional, or as a map value
- [ ] A visitor answering `Skip` at one property sees no node beneath it and every node
      elsewhere; `exit_property` runs exactly once per property descended and never for a leaf,
      `exit_node` once per node entered, in reverse nesting order
- [ ] `Skip` from each of the four callbacks prunes what the spec's section 5.1 says; `Stop`
      unwinds every open exit and returns `Stopped`; `Abort` runs no further callback and returns
      `Aborted`; a visitor error ends the walk like an `Abort` and is returned
- [ ] `to_struct` on a root and on a nested node equals the eager parse of the same bytes, over
      both trees
- [ ] `leaf()` and `map_key()` agree between the two trees for every leaf kind and every key
      kind `Kind::is_valid_map_key` admits
- [ ] `Trail::classes()` has one entry per field step at every node, equal to the class of the
      node that field was read on, and `to_value_path()?.fields()` yields the same pairs
- [ ] `Trail::to_string()` equals `to_value_path()?.to_string()` at every node; a walk over a
      map of 10,000 hash-keyed entries grows the trail's capacity by at most one step, and over
      a view allocates nothing else
- [ ] `BinOverride::walk` visits the fixture patch's embedded objects and never a record's value
- [ ] `Bin::walk` over `lolminimap_uibase.bin` visits 66 root nodes in file order, and
      `BinStream::walk` over the same bytes visits the same
- [ ] Corpus, `#[ignore]` under `LTK_LOL_GAME_DIR`: every chunk walks through `BinStream::walk`
      and `Bin::walk` with identical visit sequences, and the node count equals an independent
      count of objects plus `Struct` and `Embedded` values with a non-zero class (AC-7)
