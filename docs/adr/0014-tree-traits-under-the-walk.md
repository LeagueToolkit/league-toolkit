# ADR-0014: Tree traits under the walk

- **Status:** Accepted
- **Date:** 2026-09-02
- **Crates:** `ltk_meta`
- **Related:** PRD-001 (FR-12), PRD-002 (FR-2, FR-6), ADR-0007, ADR-0013, #225,
  `docs/design/value-walk.md` [section 3](../design/value-walk.md#s3) and
  [section 5](../design/value-walk.md#s5)

## Context and problem statement

ADR-0013 gave `ltk_meta` one single-visitor walk. The first draft ran it over the owned tree
only, and a consumer sweeping a mounted file called `read()` on each object and walked the
`BinObject`. That is streaming at the file level and eager inside the object: every property of
every object is decoded into a `PropertyValueEnum`, 96 bytes per node, whether or not the
visitor looks at it.

The crate already has the other representation. `ObjectView` and `ValueView` are the zero-copy
views over an object's buffered bytes (ADR-0007), one variant per `Kind`, descending to any
depth without materialising anything, and the manager's pass is the consumer they were built
for. A walk that could not run over them would be a second traversal the day someone wanted one,
and the manager's budget would keep an expansion factor the views make unnecessary.

The manager's own ADR-0014 chose a materialised object per visitor. It was written before the
views existed, and nothing in its rules depends on the object being owned.

## Decision drivers

- One traversal and one visitor for both representations, so a rule is written once.
- A stream pass that materialises nothing and holds one object's bytes at a time (PRD-002's
  constant-memory rule).
- A visitor that does not name `PropertyValueEnum`, `M`, or `ValueView`, so a later rework of
  the value model does not touch it.
- No third representation: the abstraction is sealed.

## Considered options

1. **Owned tree only** - `read()` per streamed object, walk the `BinObject`.
2. **Two sealed traits, `TreeNode` and `TreeValue`, implemented by both** - the walk and the
   visitor are generic over the value type.
3. **Views only** - walk `ObjectView`; an owned object is serialised to bytes and viewed.

## Decision

**Option 2. The walk is written against `TreeNode` and `TreeValue`, sealed traits that the
owned tree and the views both implement, and a `Visitor` is generic over the tree's value
type.** The traits are `docs/design/value-walk.md` [section 3](../design/value-walk.md#s3); the
entry points over both trees are [section 5](../design/value-walk.md#s5).

The traits carry only what a walk and a visitor need: kind, `holds_node`, descent into a node
or into children, a decoded `Leaf`, a decoded `MapKey`, and an owned escape hatch. The view's
implementations are fallible where a header can fail to decode, so the walk returns `Result`
over both trees.

## Consequences

- **Positive:** `BinStream::walk` is the manager's pass with no `BinObject` anywhere, and the
  same visitor verifies a repaired owned tree. `Leaf` and `MapKey` give the new surface the
  client's tag names without renaming `Kind`, and a visitor survives a value-model rework
  unchanged.
- **Negative:** a visitor is generic - `impl<'a, V: TreeValue<'a>> Visitor<'a, V> for Rule` -
  which is more to write than a concrete trait, and a `for<'a>` bound on the stream entry
  points that a consumer has to understand once. Two implementations of each trait to keep in
  step, which the parity test in [section 7](../design/value-walk.md#s7) is for. And the walk
  is fallible over the owned tree too, where it never fails.
- **Revisit when:** the value model is reworked without `M`. Then the owned tree and the view
  may collapse toward one representation and the traits toward it.

## Pros and cons of the options

### Option 1: owned tree only

- Good: the simplest walk, infallible, no traits, no generics on the visitor.
- Bad: a streamed sweep decodes everything, the views go unused by the consumer they were built
  for, and the visitor's signature names `PropertyValueEnum<M>` forever. Tempting because it is
  what the handoff and the manager's ADR asked for; but both predate the views.

### Option 3: views only

- Good: one implementation of the traversal, nothing generic.
- Bad: an owned tree has to be serialised to be walked, so a repair verifying its own edit in
  memory writes the object first; and merge and diff, which walk owned trees, get nothing from
  it.
