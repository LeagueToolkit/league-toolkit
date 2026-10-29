# ADR-0033: Container kind fields

- **Status:** Proposed
- **Date:** 2026-10-29
- **Crates:** `ltk_wwise`
- **Related:** #310, LeagueToolkit/ltk-manager#957, `docs/design/wwise-audio.md`
  [section 9.7](../design/wwise-audio.md#s9.7)

## Context and problem statement

`ltk_wwise` 0.1.0 decodes kinds 5, 6, 7 and 9 into one `Container`: an ID, a kind, a node base,
a child list and the other fields as bytes. `Typed::Container` holds it, and `event_sources`
reads only its child list. The four kinds share the node base and the child list. They differ in
the fields around the child list: a RandomSequenceContainer has a playlist, a SwitchContainer has
a group and the node list of each switch, a LayerContainer has layers, an ActorMixer has nothing.

A RandomSequenceContainer, a SwitchContainer and a LayerContainer name each child again in those
fields. A child removed from the child list and left in a playlist or a switch is a dangling
reference in the bank. A consumer that edits an event bank removes and replaces children
(LeagueToolkit/ltk-manager#957). A consumer that walks an event reads the children of every
container kind the same way.

## Decision drivers

- A walk over children reads one field for every container kind.
- The kind of a container and its fields cannot disagree.
- A child edit keeps the child list and the lists of the kind in step.

## Considered options

1. **One `Container` with a `fields` enum**, one variant per kind, and the shared node base and
   child list on the struct.
2. **One `Typed` variant per kind**, `RandomSequence`, `Switch`, `ActorMixer` and `Layer`, each
   with its own node base and child list.
3. **One `Container` with the fields as bytes** and accessors that decode the bytes on request.

## Decision

**Option 1. `Container` holds `id`, `node`, `children` and `fields: ContainerFields`. The variant
of `fields` selects the kind, and `Container::kind` reads it. `Container::remove_child` and
`Container::replace_child` edit `children` and every list of `fields` that names the child.
`children` is a public field, and a direct edit of it leaves `fields` as it is.**

`docs/design/wwise-audio.md` [section 9.7](../design/wwise-audio.md#s9.7) specifies the fields
and the child edits.

## Consequences

- **Positive:** the walk and a consumer read `children` of any container. A container of a kind
  other than 5, 6, 7 and 9 cannot be built. `Typed` has the same four variants as in 0.1.0.
- **Negative:** the kind of a container is the method `Container::kind`, where 0.1.0 has a
  public `kind` field: a break of the 0.1.0 surface. A consumer that needs the playlist matches
  on `fields` after matching on `Typed::Container`. A direct edit of `children` can leave a
  dangling reference.
- **Revisit when:** the crate decodes the music containers, kinds 12 and 13, whose node base
  differs.

## Pros and cons of the options

### Option 2: one `Typed` variant per kind

- Good: a match on `Typed` reaches the playlist in one step.
- Bad: the walk matches four variants to read one child list, and a consumer repeats that match.
  The node base and the child list repeat in four structs. `Typed::Container` disappears, a
  larger break than the `kind` field.

### Option 3: the fields as bytes with accessors

- Good: the struct keeps the 0.1.0 shape, and an object that a consumer never inspects is never
  decoded past the child list.
- Bad: an accessor that edits a playlist rewrites bytes in place and re-frames the lists after it.
  `remove_child` needs a decoder and an encoder of each kind in any case. A kind field and bytes
  of another kind can disagree, and encoding reports it only as a layout error.
