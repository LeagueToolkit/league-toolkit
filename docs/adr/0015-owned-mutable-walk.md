# ADR-0015: Owned mutable walk

- **Status:** Accepted
- **Date:** 2026-09-14
- **Crates:** `ltk_meta`
- **Related:** PRD-001 (FR-12, FR-15), ADR-0013, ADR-0014, #225,
  `docs/design/value-walk.md` [section 5.3](../design/value-walk.md#s5.3) and
  [section 6](../design/value-walk.md#s6)

## Context and problem statement

`ltk_meta` owns one read-only walk (ADR-0013). A repair edits the tree the walk reads. `ltk-manager`'s
`bin_property_type::fix` holds a hand-written mutable recursion over the six recursive variants of
`PropertyValueEnum`: `repair`, `repair_into`, `repair_map` and `repair_container`. It keeps a trail
of its own that clones every map key it descends, and renders that trail through the same `Address`
as its check. The check runs as a `Visitor` over `BinStream::walk` and verifies the repaired tree
over `BinObject::walk`. The descent the crate owns for reading is written a second time in the
consumer for writing.

The address a repair matches on is the hash form of the read-only `Trail`. A second trail type with
a second rendering is a second grammar to keep equal to the first.

The read-only `Trail<V>` borrows each map key from the tree. A mutable walk holds the map's entries
through `&mut`, and each recursion level reborrows the level above it for a shorter lifetime. A
`Vec` of key borrows that spans every level has one element lifetime. The borrow checker cannot
state that a key pushed at a deep level outlives nothing past its pop.

A container, an optional and a map pin the kind of every item they hold. `ValueSlot` enforces the
pin on a whole-value replace. A property of a node pins nothing.

## Decision drivers

- No consumer matches the six recursive variants by hand to edit a bin.
- An edit is addressed by the same `Trail` and the same hash form the read-only walk renders.
- Descending a map of ten thousand entries allocates nothing per entry, over either walk.
- An edit through the walk cannot leave a container, optional or map holding a kind it does not
  declare.
- No `unsafe` beyond what the trail requires, each block with its invariant written beside it.

## Considered options

1. **No mutable walk** - a repair keeps its own recursion; the crate owns the read-only walk alone.
2. **Resolve then mutate** - the read-only walk collects addresses; a `resolve_mut` over each
   address hands out the value to edit.
3. **A mutable visitor with a trail of its own** - `VisitorMut` over the owned tree, carrying an
   owned or stack-linked trail type beside `Trail<V>`.
4. **A mutable visitor sharing `Trail`** - `VisitorMut` over the owned tree; the walker holds
   `Trail<&PropertyValueEnum>` with each key borrow extended past the reborrow it came from, and
   a callback sees the trail only under its own borrow.

## Decision

**Option 4. `ltk_meta` owns one mutable walk over the owned tree, `walk_mut`, driven by a
`VisitorMut` that shares `Visit`, `WalkOutcome`, the traversal rules and the `Trail` type of the
read-only walk.** The surface and the rules are `docs/design/value-walk.md`
[section 5.3](../design/value-walk.md#s5.3).

A node callback edits the node's property map. A property callback edits or replaces the
property's value. No callback reaches an item of a container, optional or map as a value, and
`NodeMut` sets no class hash: every kind pin holds by construction. The walker extends a
map key's borrow in one `unsafe` block. The key is popped before the entry borrow it came from ends,
no callback reaches a map key through `&mut`, and a callback sees a key only for the length of the
callback.

## Consequences

- **Positive:** a repair is a visitor over `walk_mut` and its verification a visitor over `walk`,
  with one trail and one address between them. `Trail::classes`, the hash form and
  `Trail::to_value_path` serve both walks unchanged. Descent over a map allocates nothing.
- **Negative:** the crate's first `unsafe` block, whose soundness rests on the walker's push and
  pop discipline rather than on the borrow checker. No Miri run gates it. A property callback cannot
  read the node's other properties: the value borrow excludes them, and a visitor that needs a
  sibling reads it in `enter_node`. A mutable visitor is a second trait beside `Visitor`, and a rule
  that checks and repairs implements both.
- **Revisit when:** a value model without `M` lets the owned tree hand out keys and values apart;
  or the borrow checker expresses a stack of reborrows, and the extension becomes safe code.

## Pros and cons of the options

### Option 1: no mutable walk

- Good: no new surface and no `unsafe`; the manager's recursion exists and works.
- Bad: every consumer that edits a bin writes the six-variant recursion and a trail again, and each
  trail's rendering has to stay equal to `Trail`'s by hand. Tempting with the manager as the one
  mutating consumer. The bin editor and `Bin::merge` edit owned trees as well.

### Option 2: resolve then mutate

- Good: no mutable traversal at all; the read-only walk and a resolver compose.
- Bad: two passes per edit, an owned `ValuePath` allocated for every position edited, and a resolve
  per address that repeats descent the walk has done. An edit that changes what lies beneath it
  (retagging an embed as a pointer) invalidates addresses collected below it.

### Option 3: a trail of its own

- Good: safe code throughout. A stack-linked trail allocates nothing.
- Bad: a second public trail type with its own steps iterator, class context and rendering, and a
  second `to_value_path`. A trail that owns its keys allocates per string key. The repair matches
  on text a second renderer produces, which is the drift the shared address exists to prevent.
