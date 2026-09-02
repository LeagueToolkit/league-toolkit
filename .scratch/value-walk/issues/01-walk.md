---
issue: 225
title: "Value walk: one read-only traversal over every node of a bin"
labels: crate:ltk_meta, enhancement, format:bin, area:api, blocked
---

Design: `docs/design/value-walk.md` [section 3](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/value-walk.md#s3) and [section 5](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/value-walk.md#s5); requirements PRD-001 FR-12 and FR-13.
A `Visitor` called once per node of an object, in pre-order and file order, with a prune per
property and a borrowing `Trail` that renders to a `ValuePath` (#219) only for a node a visitor
reports on.

The consumer is `ltk-manager`'s problems pass, which reads every bin once and runs every
health-check rule over that one read. It has two hand-written walkers today, each matching the
six recursive `PropertyValueEnum` variants by hand, and a third on the way; with this they are
visitors. `Bin::merge` (#220) and `Bin::diff` build their addresses from the same `Trail`.

## Proposed surface

```rust
impl Kind {
    /// Whether a value of this kind is a node: `Struct` or `Embedded`.
    pub fn is_node(self) -> bool;
}

impl<M> PropertyValueEnum<M> {
    /// Whether entering this value can reach a node.
    ///
    /// True for a `Struct` or `Embedded` whose class hash is not 0, and for a container,
    /// optional or map whose item kind [`Kind::is_node`].
    pub fn holds_node(&self) -> bool;
}
```

In `ltk_meta::walk`:

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
    fn leaves(&mut self, field: BinHash, value: &PropertyValueEnum<M>, at: &Trail<'_, M>) {}
}

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
    pub fn is_root(&self) -> bool;
    /// The node's address, copied out of the trail. Allocates.
    pub fn value_path(&self) -> ValuePath where M: Clone;
}

/// The steps from an object's root to the walk's position. Borrows the tree; a map key is a
/// reference, never a copy.
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

impl<M> fmt::Display for Trail<'_, M> { /* the hash form, without building a ValuePath */ }

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
    /// Walks every embedded object, in file order, as the file holds them: a record that
    /// targets one of them has not been applied. Patch records are not walked.
    pub fn walk<V: Visitor<M>>(&self, visitor: &mut V);
}
```

## Rationale

**W3 (ADR-0013): one visitor, the fan-out stays with the consumer.** The descent with a trail is
the same for every consumer and is what merge and diff need; which visitors are active beneath a
value is one consumer's policy. `leaves` is on the trait so that fan-out can be one `Visitor`
that pushes its active set on `enters` and pops it on `leaves` (W8).

**Traversal** is `value-walk.md` [section 5.1](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/value-walk.md#s5.1): pre-order, file order, each node once. A `Struct` or
`Embedded` with class 0 is the client's null pointer and is not a node (W2). A visitor is asked
`enters` at properties only; the items of a container, optional or map it entered are all
descended (W7). An optional's value is `Index(0)`, as the path grammar addresses it (D9).

**The trail allocates nothing per step.** Keys are borrowed; text and owned steps are made only
by `Display`, `to_value_path` or `Node::value_path`, which a visitor calls for a node it reports
on (W9).

**`is_primitive` is not load-bearing.** The prune is `holds_node` over `Kind::is_node`; the set
`Kind::is_primitive` covers is a value-model question the walk does not ask (W1).

Blocked by #219: `Node::value_path`, `Trail::to_value_path` and `TrailStep::to_step` produce its
types. `Kind::is_node`, `holds_node`, the trait, `Node`, `Trail` and the entry points do not
depend on it and can land first behind those three methods.

- [ ] A visitor recording `(object_hash, class_hash, hash form)` per node over the fixture tree of
      `value-walk.md` [section 7](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/value-walk.md#s7) matches a hand-written pre-order list, each node once
- [ ] A `Struct` or `Embedded` with class 0 is never visited, at a property, in a container, in
      an optional, or as a map value
- [ ] A visitor that declines one property sees no node beneath it and every node elsewhere;
      `leaves` is called exactly once per `enters` that returned `true`, in reverse nesting order
- [ ] `Trail::classes()` has one entry per field step at every node, equal to the class of the
      node that field was read on, and `to_value_path().fields()` yields the same pairs
- [ ] `Trail::to_string()` equals `to_value_path().to_string()` at every node, and a walk over a
      map of 10,000 hash-keyed entries grows the trail's capacity by at most one step
- [ ] `BinOverride::walk` visits the fixture patch's embedded objects and never a record's value
- [ ] `Bin::walk` over `lolminimap_uibase.bin` visits 66 root nodes in file order
- [ ] Corpus, `#[ignore]` under `LTK_LOL_GAME_DIR`: every object in the install walks, and the
      node count equals an independent count of objects plus `Struct` and `Embedded` values with a
      non-zero class (AC-7)
