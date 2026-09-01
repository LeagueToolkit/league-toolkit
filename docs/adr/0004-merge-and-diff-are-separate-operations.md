# ADR-0004: Merge and diff are separate operations

- **Status:** Accepted
- **Date:** 2026-08-31
- **Crates:** `ltk_meta`
- **Related:** PRD-001 (FR-7, FR-10), `ltk-manager` ADR-0012, #218, #220, #221,
  `docs/design/ptch-property-patches.md` [section 10](../design/ptch-property-patches.md#s10) and
  [section 12](../design/ptch-property-patches.md#s12)

## Context and problem statement

A mod arrives in one of two shapes: an edited `PROP` bin, which is what mods ship today, or a
`PTCH` authored against the install it was made on. Both have to end at one merged bin in the
overlay.

The tempting shape is one operation. Diff the two bins into a `BinOverride`, then `apply` it, and
both input paths converge on the record language with a single set of semantics to test.

It does not hold, for a reason that belongs to the format rather than to the implementation.
**A record cannot insert a map entry.** `patch_in` creates a leaf only when the last segment names
it outright and its parent is an object, a `Struct` or an `Embed`; a `{key}` subscript needs an
entry to subscript. A mod that adds 84 map keys has no record set that says so. The nearest
expressible record carries the whole map as one value - which is exactly the wholesale replacement
ADR-0012 exists to stop.

## Decision drivers

- The overlay repair has to be **total**: a resolver miss can crash and nothing readable says
  which keys are dangerous.
- Do not invent semantics the format does not have.
- Share one walk if the two operations can share one, so their answers cannot drift.

## Considered options

1. **One operation** - diff into a `BinOverride`, then apply it.
2. **Two operations over one walk** - `merge` in process, `diff` rendering as much of the same
   walk as records can carry.
3. **Merge only** - never express a difference as a patch.

## Decision

**Option 2.**

`merge` applies in process with no serialization constraint, so it can insert map entries and
anything else the walk reaches. `diff` renders as much of the same walk as records can carry and
reports every place it could not, so a caller learns what a patch would lose before it ships one.
[Section 10](../design/ptch-property-patches.md#s10) and
[section 12](../design/ptch-property-patches.md#s12) specify both, including the invariant that ties
them and holds wherever `diff` lifted nothing.

Two rules follow, and are stated there with them: containers replace whole, with no element-wise
merge and no LCS, because a list has no key to combine by and a positional merge would invent a
meaning the format does not have (D22); and `diff` is written down but parked until an authoring
flow needs it (D27).

## Consequences

- **Positive:** the overlay repair is total, because `merge` is not limited to what a record can
  say. Nothing silently degrades a map insert into a whole-map replacement.
- **Negative:** two surfaces that must stay in step. The invariant above is the test that keeps
  them there, and it only holds where `diff` lifted nothing.
- **Revisit when:** the record language gains a way to insert a map entry - which would need a
  client change, not a toolkit one.

## Pros and cons of the options

### Option 1: one operation

- Good: one set of semantics; the record language is the only thing to test.
- Bad: silently converts a map insert into a whole-map replacement, which is the defect the
  consumer is trying to fix. Wrong answers, cheaply.

### Option 3: merge only

- Good: smallest surface.
- Bad: gives up expressing a mod as a patch at all, which PRD-001's third delivery route and every
  authoring flow need.
