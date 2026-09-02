# ADR-0013: Single-visitor walk

- **Status:** Accepted
- **Date:** 2026-09-02
- **Crates:** `ltk_meta`
- **Related:** PRD-001 (FR-12, FR-13), ADR-0005, ADR-0012,
  `docs/design/value-walk.md` [section 5](../design/value-walk.md#s5) and
  [section 6](../design/value-walk.md#s6)

## Context and problem statement

Reading a bin for what it contains means recursing over the six recursive variants of
`PropertyValueEnum` - `Struct`, `Embedded`, `Container`, `UnorderedContainer`, `Optional`,
`Map` - while keeping a stack of where you are, so that a finding can say where it was found.
`ltk-manager` has two hand-written copies of that recursion today (`bank_units::walk` and
`bin_property_type::walk`), its problems pass is about to need a third that runs several rules
over one traversal, and inside this crate `Bin::merge` and `Bin::diff` each need one more.

`ltk_meta` has no read-only traversal. It has `stream::layout`'s skip walker, which crosses
*bytes* by declared size and never sees a decoded value, and it has `PropertyPath::resolve`,
which follows one path and visits nothing beside it. Neither serves a consumer that wants to be
called at every node.

The question is how much of the traversal belongs in the crate. The manager's design carries a
set of visitors down the recursion, each with its own prune, entering a value if any active
visitor wants it and calling each only where it asked to be. That is real logic, and it is
tempting to give it a home here so no consumer writes it twice.

## Decision drivers

- No consumer should match the six variants by hand to read a bin.
- The trail and the address must be one thing across the walk, merge and diff, so a position
  means the same wherever it was reported (ADR-0005).
- Nothing in the crate's public surface should encode one consumer's scheduling policy.
- A visitor over a map of ten thousand entries must not allocate per entry.

## Considered options

1. **A predicate and a step type only** - `holds_node`, `Step`, `ValuePath`; every consumer keeps
   its own recursion.
2. **A single-visitor walk with a trail** - one `Visitor` with `visit`, `enters` and `leaves`;
   the crate owns the descent and the trail; anything that fans out to several visitors is built
   on top of it as one `Visitor`.
3. **A multi-visitor walk** - the crate takes a slice of visitors, carries the active set, and
   applies each visitor's prune.
4. **An iterator of nodes** - `nodes()` yielding `(Trail, Node)` with no callbacks.

## Decision

**Option 2. `ltk_meta` owns one single-visitor, read-only, pre-order walk with a borrowing
trail, and the multi-visitor active set stays with the consumer.** The traversal is
`docs/design/value-walk.md` [section 5.1](../design/value-walk.md#s5.1); the trail is
[section 5.2](../design/value-walk.md#s5.2); what merge and diff share with it is
[section 6](../design/value-walk.md#s6).

`leaves` is part of the trait precisely so that a consumer's fan-out can be one `Visitor`: it
pushes its active set on `enters` and pops it on `leaves`, and the crate never learns that
there was more than one.

## Consequences

- **Positive:** the manager's two walkers and its planned third collapse into visitors, and the
  manager's fan-out is a small adapter over a walk it does not maintain. Merge and diff build
  their addresses from the same `Trail`, so `Replaced::at` and a manager finding render the same
  way for the same position.
- **Negative:** two traversals in the crate - the byte skipper in `stream::layout` and this walk
  over decoded values - at different layers, with ADR-0008's "one layout core" applying to the
  first and not the second. (Corrected 2026-09-02: the walk runs over the views as well as the
  owned tree, ADR-0014, so over a stream it is a traversal of the same buffered bytes the
  skipper crosses, not a second decode.) And a call per entered property for `leaves` that most
  visitors ignore. The walk also has no early exit; a search that wants to stop at the first hit declines
  every further `enters` instead, which is a cost only if such a search appears.
- **Revisit when:** a second consumer wants the same fan-out the manager built. Then the
  active-set adapter is generic and belongs beside the walk.

## Pros and cons of the options

### Option 1: predicate and step only

- Good: the smallest surface, nothing to pin under semver.
- Bad: every consumer still matches six variants by hand, and merge and diff still write their
  own trails - the duplication is unchanged, only the vocabulary is shared.

### Option 3: a multi-visitor walk

- Good: the manager's fan-out lands once, in the crate.
- Bad: which visitors are active beneath a value, and what a visitor is handed to report into,
  are the manager's policy; a slice of trait objects and a sink type would pin that shape for
  every consumer. Tempting because the logic is genuinely reusable; but it is one adapter over
  option 2, and can be lifted here later without a break.

### Option 4: an iterator of nodes

- Good: no trait to implement; `for node in object.nodes()` reads well.
- Bad: each yielded node borrows the trail, so it cannot be a `std` iterator, and pruning a
  subtree needs a call back into the iterator between items. That is a visitor with worse
  ergonomics.
