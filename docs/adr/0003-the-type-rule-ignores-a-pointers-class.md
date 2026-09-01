# ADR-0003: The type rule compares value shape and ignores a pointer's class

- **Status:** Accepted
- **Date:** 2026-08-24 (corrects an earlier reading of the client)
- **Crates:** `ltk_meta`
- **Related:** PRD-001 (FR-4), ADR-0006, #173,
  `docs/design/ptch-property-patches.md` [section 9.3](../design/ptch-property-patches.md#s9.3)

## Context and problem statement

Applying a record means deciding whether its value may overwrite the leaf the path names. The
client decides with an exact tag comparison: `MetaValue_readInto` accepts a value whose type tag
equals the destination's and performs no coercion of any kind, and
`PropertyPatch_readAndApply` applies the identical rule from the other side. A value that does not
match is consumed from the reader and discarded, with no error and no log line.

Two composite kinds carry a class as well as a tag, and the client treats them differently:

- **An embedded struct** is compared by `MetaClass` pointer. The class must be exact.
- **A pointer** is not. The client's reader walks the primary base chain and then the
  secondary-base pairs, so a class that *derives from* the declared one is accepted and
  constructed as the file's class; an unrelated or unresolvable class is skipped. It is an is-a
  test, not an absence of one.

That is-a test needs the class hierarchy, and the class hierarchy is in the game, not in the file.
The crate has the file.

## Decision drivers

- Reproduce the client wherever the file alone contains enough to do it.
- Never silently write a value of the wrong type into a leaf.
- Require no schema (ADR-0006).

## Considered options

1. **Compare the pointer's class exactly** - treat a derived class as a mismatch.
2. **Compare shape and omit the pointer's class** - accept strictly more than the client.
3. **Take a class hierarchy** and run the is-a test properly.

## Decision

**Option 2. `ValueShape` compares kind, item kind and key kind, with an Embed's class exact and a
Pointer's class ignored.**

What `ValueShape` compares, and the client behaviour it approximates, are specified in
[section 9.3](../design/ptch-property-patches.md#s9.3).

## Consequences

- **Positive:** the rule runs on the file alone, embed exactness - the half the file *can* decide -
  is preserved, and the crate never rejects a pointer write the client would have accepted.
- **Negative:** the crate accepts a pointer write the client would skip. A patch that `check`
  calls clean can still have a pointer record silently discarded in game. The gap is bounded to
  pointer-typed records and is documented on the type.
- **Revisit when:** the crate can see the class hierarchy - a schema-holding caller, or a hierarchy
  loaded from a dump. `ValueShape` is the single comparison point, so option 3 becomes available
  without changing the surface around it.

## Pros and cons of the options

### Option 1: exact pointer class

- Good: no false accepts.
- Bad: false rejects instead, and they are the common case - a pointer property declared as a base
  class and written with a derived one is ordinary in shipped data. Rejecting those would make
  `check` useless on real patches.

### Option 3: take a hierarchy

- Good: exactly reproduces the client.
- Bad: puts build-versioned Riot data inside a format crate, which ADR-0006 rules out.
