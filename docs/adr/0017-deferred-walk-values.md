# ADR-0017: Deferred walk values

- **Status:** Accepted
- **Date:** 2026-09-15
- **Crates:** `ltk_meta`
- **Related:** ADR-0014, #225, `docs/design/value-walk.md`
  [section 3](../design/value-walk.md#s3)

## Context and problem statement

`ValueView` holds decoded leaves. A `ValueView` built for a property validates its string before
the visitor can skip the property. The walk contract lets a visitor read a kind without decoding
a leaf.

## Considered options

1. **Deferred walk adapter.** `RawValue` keeps a property view until a call asks for a decoded
   value.
2. **Deferred streaming enum.** Change `ValueView` variants to hold deferred payloads.
3. **Eager property callbacks.** Decode every leaf before the visitor call.

## Decision

**Use a deferred walk adapter.** `RawValue` implements the streaming side of `TreeValue`.
The surface is specified in [section 3](../design/value-walk.md#s3).

## Consequences

- **Positive:** A skipped property's leaf needs no decoding. The streaming enum keeps its
  variants. A generic visitor keeps its signature.
- **Negative:** A visitor that names the streaming value type names `RawValue`. The walk has
  one more borrowed adapter type.
- **Revisit when:** The streaming enum itself defers leaf decoding.

## Pros and cons of the options

### Deferred streaming enum

- Good: one borrowed value representation.
- Bad: every consumer that matches a decoded variant needs migration.

### Eager property callbacks

- Good: no extra adapter and no API migration.
- Bad: a skipped malformed string fails the walk. Every property string costs UTF-8 validation.
