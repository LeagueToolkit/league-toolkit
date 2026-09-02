# ADR-0012: Field step class

- **Status:** Accepted
- **Date:** 2026-09-02
- **Crates:** `ltk_meta`
- **Related:** PRD-001 (FR-7, FR-8, FR-13), ADR-0005, #219,
  `docs/design/value-walk.md` [section 4.1](../design/value-walk.md#s4.1) and
  [section 4.3](../design/value-walk.md#s4.3)

## Context and problem statement

ADR-0005 settled that a report addresses a position by hash, with `ValuePath`. It left open
what a field step carries, and the first draft of #219 carried the field hash alone:
`Step::Field(BinHash)`, named through `FieldNames::field(hash)`.

The consumer that will render these addresses does not hold a table keyed that way.
`ltk-manager`'s problems pass renders an address in two forms, a stable one for a repair to
match on and a readable one for a user, and both are produced from tables keyed by **class**:
`lol-meta-classes` dumps one meta class at a time, with the fields of each under it, and the
manager's migration tables name a field by the class it belongs to. To spell a field hash from
such a table you need the class of the node it was read on, and by the time a report is being
rendered the tree that would tell you is gone. The manager's own trail therefore already carries
`Field { class, field }`, and if `ValuePath` did not, the manager would keep its own address
type beside the toolkit's, which is the duplication this work exists to remove.

The decision has to land before #219 does: a `Step` variant's shape is public API, and every
report type in `ptch-property-patches.md` sections 10 and 12 carries one.

## Decision drivers

- One address type for the toolkit's reports and the manager's, so a position means the same
  thing whoever produced it.
- A name lookup that works against the tables consumers actually hold.
- Nothing in the rendered text that a name table could move: the class is context, not address.

## Considered options

1. **`Field(BinHash)`** - the field hash alone; a consumer that needs the class keeps its own
   trail.
2. **`Field { class, field }`** - the class of the node the field was read on rides on the step.
3. **Class per node, not per step** - a parallel list of classes on `ValuePath`, one per field
   step, kept out of `Step`.

## Decision

**Option 2. Every `Step::Field` carries the class hash of the node the field is on, and
`FieldNames::field` takes the class as its first parameter.** The rule is
`docs/design/value-walk.md` [section 4.1](../design/value-walk.md#s4.1); the lookup contract is
[section 4.3](../design/value-walk.md#s4.3).

The class is context for the name table and nothing else: the hash form of a path does not
print it, so two paths through different classes to the same field hashes render the same, as
they should.

## Consequences

- **Positive:** `ValuePath` is the manager's address type as well as the toolkit's. A
  class-keyed table answers directly; a field-keyed table ignores the argument and loses
  nothing.
- **Negative:** four more bytes per field step, and a `ValuePath` can no longer be built from a
  `PropertyPath` alone - the class of each node comes from the tree, so the conversion in that
  direction is a resolution against an object, not a rewrite of the text. Nothing needs that
  conversion today.
- **Revisit when:** a class-independent field-name table becomes the one every consumer holds.
  Then the class is dead weight on the step, though harmless.

## Pros and cons of the options

### Option 1: the field hash alone

- Good: the smaller step; a `HashMap<BinHash, String>` is the whole name table.
- Bad: it cannot be named from a per-class table without scanning every class, and the
  consumer that needs it would keep a second address type. Tempting because FNV-1a of a field
  name is the same on every class, so a flat table *can* be built; but nobody ships one, and
  building it flattens away the disambiguation a class gives.

### Option 3: classes beside the steps

- Good: `Step` stays as drafted.
- Bad: two sequences to keep in step, an invariant `push` and `pop` have to maintain, and a
  `Step` handed out on its own no longer says what class it was on.
