---
issue: 237
title: "Mutable walk: edit every node of an owned bin in one traversal"
labels: crate:ltk_meta, enhancement, format:bin, area:api, blocked
---

Design: `docs/design/value-walk.md` [section 5.3](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/value-walk.md#s5.3); requirement PRD-001 FR-15; decision ADR-0015.
A `VisitorMut` called at every node and property of an owned object, in the order and with the
answers of the read-only walk (#225), editing a node's property map and a property's value in
place, under the same `Trail`. A repair edits with it and verifies with the read-only `Visitor`
over the same tree, and the address the check records is the address the repair matches on.

The consumer is `ltk-manager`'s `bin_property_type::fix`, whose hand-written mutable recursion
(`repair`, `repair_into`, `repair_map`, `repair_container`) clones every map key into a trail of
its own. With this it is one `VisitorMut` over `BinObject::walk_mut`, and over a mounted file it
reads only the objects it edits and saves through `BinStream::write_patched` (#211).

## Proposed surface

In `ltk_meta::walk`:

```rust
/// What a mutable walk calls. The owned tree only. Every callback defaults to `Continue`.
pub trait VisitorMut<M = NoMeta> {
    type Error: From<Error>;

    fn enter_node(&mut self, node: &mut NodeMut<'_, M>) -> Result<Visit, Self::Error>;
    fn exit_node(&mut self, node: &mut NodeMut<'_, M>) -> Result<Visit, Self::Error>;
    fn enter_property(&mut self, property: &mut PropertyMut<'_, M>) -> Result<Visit, Self::Error>;
    fn exit_property(&mut self, property: &mut PropertyMut<'_, M>) -> Result<Visit, Self::Error>;
}

impl<M, W: VisitorMut<M> + ?Sized> VisitorMut<M> for &mut W {}

pub struct NodeMut<'t, M = NoMeta> { /* object hash, class hash, &'t mut IndexMap, &'t Trail */ }

impl<'t, M> NodeMut<'t, M> {
    pub fn object_hash(&self) -> BinHash;
    pub fn class_hash(&self) -> BinHash;
    pub fn trail(&self) -> &'t Trail<&'t PropertyValueEnum<M>>;
    pub fn is_root(&self) -> bool;
    pub fn inner(&self) -> OwnedNode<'_, M>;
    pub fn properties(&self) -> &IndexMap<BinHash, PropertyValueEnum<M>>;
    pub fn properties_mut(&mut self) -> &mut IndexMap<BinHash, PropertyValueEnum<M>>;
}

pub struct PropertyMut<'t, M = NoMeta> { /* object hash, node class, field, &'t mut value, &'t Trail */ }

impl<'t, M> PropertyMut<'t, M> {
    pub fn object_hash(&self) -> BinHash;
    pub fn node_class_hash(&self) -> BinHash;
    pub fn field(&self) -> BinHash;
    pub fn trail(&self) -> &'t Trail<&'t PropertyValueEnum<M>>;
    pub fn value(&self) -> &PropertyValueEnum<M>;
    pub fn value_mut(&mut self) -> &mut PropertyValueEnum<M>;
}

impl<M> BinObject<M> {
    pub fn walk_mut<W: VisitorMut<M>>(&mut self, visitor: &mut W) -> Result<WalkOutcome, W::Error>;
}
impl<M> Bin<M> {
    pub fn walk_mut<W: VisitorMut<M>>(&mut self, visitor: &mut W) -> Result<WalkOutcome, W::Error>;
}
impl<M> BinOverride<M> {
    pub fn walk_mut<W: VisitorMut<M>>(&mut self, visitor: &mut W) -> Result<WalkOutcome, W::Error>;
}
```

## Rationale

**One traversal, two borrows (ADR-0015).** The traversal is `value-walk.md` [section 5.1](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/value-walk.md#s5.1) callback for
callback. The walk iterates the property map `enter_node` leaves and descends the value
`enter_property` leaves (W26).

**The trail is `Trail`.** The walker holds each map key's borrow past the reborrow it came from, in
one `unsafe` block, and a callback sees a key only for its own length (W24). No trail step
allocates.

**Kind pins hold by construction (W25).** A node callback edits the property map, a property
callback edits the value, `NodeMut` sets no class hash, and no callback reaches an item of a
container, optional or map as a value.

**Owned only (W23).** A view's bytes are not editable in place; the unit of an edit is the object
`read()` returns.

Blocked by #225: `Visit`, `WalkOutcome`, `Trail` and `OwnedNode` are its types.

- [ ] A `VisitorMut` that edits nothing and records every callback produces the same event list as
      the recording `Visitor` over the `value-walk.md` [section 7](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/value-walk.md#s7) fixture, for every answer the flow tests
      give, and leaves the tree equal to the fixture
- [ ] A property inserted in `enter_node` is walked; a property removed in `enter_node` is not
- [ ] A value replaced in `enter_property` is descended as replaced, and a node value replaced by a
      leaf gets no `exit_property`
- [ ] An edit made through `NodeMut` at a node inside a container, optional and map writes, re-reads
      equal, and leaves every kind pin intact
- [ ] `Trail::to_string()` and `Trail::classes()` at every node equal the read-only walk's at the
      same node
- [ ] A walk over a map of 10,000 hash-keyed entries grows the trail's capacity by at most one step
- [ ] `Skip` from each callback, `Stop` and `Abort` end the walk as the read-only walk does, and a
      visitor error ends it like an `Abort` and is returned
- [ ] `BinOverride::walk_mut` edits the fixture patch's embedded objects and leaves every record
      unchanged
- [ ] Corpus, `#[ignore]` under `LTK_LOL_GAME_DIR`: `Bin::walk_mut` of every chunk with a recording
      `VisitorMut` produces the same visit sequence as `Bin::walk`
