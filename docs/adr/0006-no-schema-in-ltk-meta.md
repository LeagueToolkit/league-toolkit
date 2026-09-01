# ADR-0006: `ltk_meta` holds no schema

- **Status:** Accepted
- **Date:** 2026-08-31
- **Crates:** `ltk_meta`
- **Related:** PRD-001 (out of scope), ADR-0003, #218, #221, #222 (dropped),
  `docs/design/ptch-property-patches.md` [section 2](../design/ptch-property-patches.md#s2) and
  [section 14](../design/ptch-property-patches.md#s14)

## Context and problem statement

`lol-meta-classes` dumps every build's meta classes - each class's properties, their types, the
base chain and their default values - versioned per build from 13.15 onward. It exists, it is
ours, and it answers questions this design had previously called unanswerable:

- Is this record a no-op, because it sets a property to the value the class defaults to?
- Has this property's type migrated since the mod was authored?
- Does this pointer's class derive from the declared one (ADR-0003)?

The question is whether `ltk_meta` should take a schema to answer them.

The vocabulary this turns on - **class**, **meta class**, **schema** - is defined in the spec,
[section 2](../design/ptch-property-patches.md#s2). This record uses those three words in exactly
that sense and no other.

## Decision drivers

- The crate must work with no dump present. It is a format crate; a file is all it is given.
- A dump is build-versioned data. Data that goes stale on a patch cycle does not belong inside a
  library that ships on a release cycle.
- The client's apply is the contract being reproduced, and the client's apply consults reflection
  the file does not carry - but it is *the client's* reflection, not a linting pass.

## Considered options

1. **A schema trait** taken by `apply_with` and a `strip_noops` inside the crate.
2. **No schema. Expose per-record outcomes** and let a caller that has a dump do the judging
   outside.
3. **Vendor the dump** into the crate for the builds it knows.

## Decision

**Option 2. Reproducing the client's apply is `ltk_meta`'s work; judging a mod against Riot's meta
classes is not.**

The surface a caller needs to do the judging outside is `ApplyReport::outcomes` - what each record
did, in file order - and `BinOverride::retain_with`, which drops the records a caller rejects.
Stripping runs as a post-pass over a finished `BinOverride`. Section
[section 14](../design/ptch-property-patches.md#s14) specifies that surface.

Two consequences that follow directly and are decided here rather than left open:

**ADR-0003's pointer gap and the missing-intermediate skip stand as taken.** With the base chain in
reach both could now be closed, and neither is: closing them is exactly the schema dependency this
ADR refuses.

**There is no baseline.** An earlier draft proposed capturing every record's authored-over value so
a later build could detect that the base had changed underneath it. Dropped, for two reasons. A mod
is authoritative where it speaks - an author who set `Anchor = (0,1)` wants `(0,1)` whatever Riot
moved it to - so that report would fire on every record of every mod after every patch. And the
failure that does hurt, a type migration, is caught from the dump with nothing captured at all.

## Consequences

- **Positive:** no build-versioned data inside a format crate, and no consumer is forced to have a
  dump to read, write, apply or merge a patch. The crate's contract is the client's behaviour,
  which is testable against shipped files.
- **Negative:** the crate cannot tell a caller that a record says nothing, and every caller that
  wants that has to hold a dump and write the same post-pass. If two of them appear, the shared
  code belongs in a new crate above this one, not inside it.
- **Sharp edge for whoever writes that post-pass:** only one of the two no-op cases needs a schema
  at all, and getting the other one wrong silently reverts a mod. Section
  [section 14](../design/ptch-property-patches.md#s14) states all three cases, and nobody should
  write the pass without reading it.
- **Revisit when:** a second consumer needs the same schema-aware pass. That is a signal to build
  the crate above, not to move the schema down into this one.

## Pros and cons of the options

### Option 1: a schema trait inside the crate

- Good: one call does everything; the crate could close ADR-0003's pointer gap exactly.
- Bad: every consumer now reasons about which build's schema it holds. The crate grows a second
  job - linting a mod - whose failure modes have nothing to do with parsing a file.

### Option 3: vendor the dump

- Good: works offline with no caller setup.
- Bad: ties a library release to a game patch cycle, and the dump is far larger than the crate.
