# ADR-0017: Deferred walk values

- **Status:** Accepted
- **Date:** 2026-09-15
- **Crates:** `ltk_meta`
- **Related:** ADR-0014, #225, `docs/design/value-walk.md`
  [section 3](../design/value-walk.md#s3)

## Context and problem statement

`ValueView` contains decoded leaves. Constructing it for a property validates strings before
the visitor can skip the property. The walk contract permits inspecting a kind without
decoding a leaf.

## Considered options

1. **Deferred walk adapter.** `ViewValue` retains a property view until decoding is requested.
2. **Deferred streaming enum.** Change `ValueView` variants to hold deferred payloads.
3. **Eager property callbacks.** Require valid leaves before calling the visitor.

## Decision

**Use a deferred walk adapter.** `ViewValue` implements the streaming side of `TreeValue`.
The surface is specified in [section 3](../design/value-walk.md#s3).

## Consequences

- **Positive:** Skipped property leaves require no decoding. The streaming enum retains its
  existing variants. Generic visitors retain their signatures.
- **Negative:** Visitors naming the streaming value type must name `ViewValue`. The walk has
  an additional borrowed adapter type.
- **Revisit when:** The streaming enum itself supports deferred leaf decoding.

## Pros and cons of the options

### Deferred streaming enum

- Good: one borrowed value representation.
- Bad: every consumer matching a decoded variant requires migration.

### Eager property callbacks

- Good: no additional adapter or API migration.
- Bad: skipped malformed strings fail the walk; every property string incurs UTF-8 validation.
