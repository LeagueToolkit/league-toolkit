# ADR-0018: Borrowed walk value access

- **Status:** Accepted
- **Date:** 2026-09-15
- **Crates:** `ltk_meta`
- **Related:** ADR-0017, `docs/design/value-walk.md`
  [section 3](../design/value-walk.md#s3)

## Context and problem statement

Manager's type checks read declared item kinds, map key kinds, counts and null class hashes.
Empty containers carry these declarations without child values. Owned conversion decodes and
allocates the complete subtree. The streaming views expose the declarations directly.

## Considered options

1. **Borrowed view access.** One explicit conversion exposes the existing streaming view API.
2. **Header methods on tree traits.** Dedicated methods expose each declaration over both trees.

## Decision

**A walk value exposes its borrowed streaming view on request.** The surface and decoding
contract are specified in [section 3](../design/value-walk.md#s3).

## Consequences

- **Positive:** consumers inspect declarations without materializing subtrees
- **Negative:** concrete streaming consumers depend on the streaming enum
- **Revisit when:** generic visitors require these declarations in the shared tree contract

## Pros and cons of the options

### Borrowed view access

- Good: one method reuses the streaming API
- Bad: requesting a leaf view decodes that leaf

### Header methods on tree traits

- Good: generic visitors share declaration queries
- Bad: the tree contract gains several format-specific methods
