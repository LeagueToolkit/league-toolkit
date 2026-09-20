# ADR-0018: Borrowed walk value access

- **Status:** Accepted
- **Date:** 2026-09-15
- **Crates:** `ltk_meta`
- **Related:** ADR-0017, `docs/design/value-walk.md`
  [section 3](../design/value-walk.md#s3)

## Context and problem statement

The manager's type checks read declared item kinds, map key kinds, counts and null class hashes.
An empty container carries these declarations and holds no child value. An owned conversion
decodes and allocates the whole subtree. The streaming views expose the declarations directly.

## Considered options

1. **Borrowed view access.** One explicit conversion exposes the streaming view API.
2. **Header methods on tree traits.** Dedicated methods expose each declaration over both trees.

## Decision

**A walk value exposes its borrowed streaming view on request.** The surface and decoding
contract are specified in [section 3](../design/value-walk.md#s3).

## Consequences

- **Positive:** a consumer reads a declaration without materializing a subtree
- **Negative:** a concrete streaming consumer depends on the streaming enum
- **Revisit when:** a generic visitor needs these declarations in the shared tree contract

## Pros and cons of the options

### Borrowed view access

- Good: one method reuses the streaming API
- Bad: a request for a leaf view decodes that leaf

### Header methods on tree traits

- Good: generic visitors share declaration queries
- Bad: the tree contract gains several format-specific methods
