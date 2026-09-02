# ADR-0012: Path class context

- **Status:** Accepted
- **Date:** 2026-09-02
- **Crates:** `ltk_meta`
- **Related:** PRD-001 (FR-7, FR-8, FR-13), ADR-0005, #219,
  `docs/design/value-walk.md` [section 4.1](../design/value-walk.md#s4.1) and
  [section 4.3](../design/value-walk.md#s4.3)

## Context and problem statement

ADR-0005 settled that a report addresses a position by hash, with `ValuePath`. It left open
where the class of each node on the way comes from when the address is rendered, and the first
draft of #219 answered "nowhere": `Step::Field(BinHash)`, named through `FieldNames::field(hash)`.

The consumer that will render these addresses does not hold a table keyed that way.
`ltk-manager`'s problems pass renders an address in two forms, a stable one for a repair to
match on and a readable one for a user, and both are produced from tables keyed by **class**:
`lol-meta-classes` dumps one meta class at a time, with the fields of each under it, and the
manager's migration tables name a field by the class it belongs to. To spell a field hash from
such a table you need the class of the node it was read on, and by the time a report is being
rendered the tree that would tell you is gone. The manager's own trail therefore already carries
the class beside every field, and if `ValuePath` could not, the manager would keep its own
address type beside the toolkit's, which is the duplication this work exists to remove.

The decision has to land before #219 does: how a path carries its classes is public API, and
every report type in `ptch-property-patches.md` sections 10 and 12 carries one.

## Decision drivers

- One address type for the toolkit's reports and the manager's, so a position means the same
  thing whoever produced it.
- A name lookup that works against the tables consumers actually hold.
- Nothing in the rendered text that a name table could move: the class is context, not address.
- `Step` stays the address and nothing else, so a path can be built from steps alone and the
  context can grow without touching it.

## Considered options

1. **`Field(BinHash)` and nothing else** - the field hash alone; a consumer that needs the class
   keeps its own trail.
2. **`Field { class, field }`** - the class of the node the field was read on rides on the step.
3. **Class context beside the steps** - `ValuePath` keeps one class per field step next to its
   steps; `Step` stays `Field(BinHash)`.

## Decision

**Option 3. A `ValuePath` keeps a class context - the class hash of the node each field was
read on - beside its steps, and `FieldNames::field` takes that class as an `Option`.** The rule
is `docs/design/value-walk.md` [section 4.1](../design/value-walk.md#s4.1); the lookup contract
is [section 4.3](../design/value-walk.md#s4.3).

The context is for the name table and nothing else: two paths with the same steps are equal
whatever their classes, and the hash form of a path does not print them. A path built from
steps alone has no context and asks the table with `None`.

## Consequences

- **Positive:** `ValuePath` is the manager's address type as well as the toolkit's. A
  class-keyed table answers directly; a field-keyed table ignores the argument and loses
  nothing. `Step` is unchanged from the #219 draft, so a path can be built from a `PropertyPath`
  or by hand without a tree, and whatever else a renderer may one day want per node - the
  object hash, a span - has a place to go that is not the address.
- **Negative:** an invariant to keep - one class per field step - that `push_field`, `pop` and
  `FromIterator` maintain and a hand-built path can get wrong only by going through them. And a
  `Step` handed out on its own no longer says what class it was on; the path does.
- **Revisit when:** a class-independent field-name table becomes the one every consumer holds.
  Then the context is dead weight, though harmless.

## Pros and cons of the options

### Option 1: the field hash alone

- Good: the smallest type; a `HashMap<BinHash, String>` is the whole name table.
- Bad: it cannot be named from a per-class table without scanning every class, and the
  consumer that needs it would keep a second address type. Tempting because FNV-1a of a field
  name is the same on every class, so a flat table *can* be built; but nobody ships one, and
  building it flattens away the disambiguation a class gives.

### Option 2: the class on the step

- Good: a step is self-describing, and there is no invariant to keep.
- Bad: the class is not part of the address, and putting it on the step says it is - it shows
  up in `PartialEq`, in every pattern match, and in every place a path is built without a tree,
  which then has to invent a class. The step type becomes the place every future per-node fact
  lands.
