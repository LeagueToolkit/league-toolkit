# ADR-0005: `ValuePath` addresses a position by hash; `PropertyPath` stays the client's language

- **Status:** Accepted
- **Date:** 2026-08-31
- **Crates:** `ltk_meta`
- **Related:** PRD-001 (FR-7, FR-8), #219, #220,
  `docs/design/ptch-property-patches.md` [section 11](../design/ptch-property-patches.md#s11)

## Context and problem statement

A merge report has to say **where**. The crate already has an address type - `PropertyPath` - but
it is the client's path language, and it cannot do this job.

`PropertyPath` is text. A segment's `name_hash()` is FNV-1a of the literal text it holds, so
writing one requires the plaintext property name, which a bin does not carry. Worse, the positions
a merge report needs to name include ones that have no name at all: a container element, a map
entry. There is no `PropertyPath` for "element 3 of the list at field `0x1e6ba0c4`" that the
client would resolve the way the report means it.

## Decision drivers

- **Totality.** Every position the merge walk can reach must be addressable, or the report has
  holes exactly where the interesting cases are.
- Keep `PropertyPath`'s promise intact: every one of them is something the client can resolve.
- Never emit text the client would hash as text and resolve somewhere else.

## Considered options

1. **A hash escape in the path grammar** - allow `#1234abcd` or `0x1234abcd` as a segment.
2. **A separate hash-addressed type** - `ValuePath`, built from `Step`s.
3. **Report by index into a flattened walk** - no address type at all.

## Decision

**Option 2. `ValuePath` is a separate type, built from `Step`s that address by hash and by
position.**

It earns its place by being **total**: every position has one, including the elements and entries
that have no name. `PropertyPath` remains the *export* language - what a patch file carries and
what the client resolves - and the two are converted, not conflated.

## Consequences

- **Positive:** a report can name any position the walk reached. `PropertyPath::new` keeps
  rejecting everything the client would reject, with no escape hatch that produces unresolvable
  text.
- **Negative:** two address types in one crate, and rendering a `ValuePath` as something a human
  reads needs a hashtable. That costs the primary consumer nothing - `ltk-manager` ADR-0009
  already gates its health check on having hashtables - but it is a real dependency for anyone
  else, and it was wrong to have once claimed otherwise.
- **Revisit when:** nothing foreseeable. The two types answer different questions.

## Pros and cons of the options

### Option 1: a hash escape in the grammar

- Good: one type, one syntax.
- Bad: breaks `PropertyPath::new`. Both `0x1234abcd` and `#1234abcd` are legal property *names*
  today, so an escape is ambiguous with real data. And a path holding an escape is text the client
  would hash as text, producing a path that resolves to the wrong place rather than failing.

### Option 3: index into a flattened walk

- Good: no new type at all.
- Bad: the index means nothing outside the one walk that produced it, so a report cannot be stored,
  compared across runs, or acted on by anything but its own caller.
