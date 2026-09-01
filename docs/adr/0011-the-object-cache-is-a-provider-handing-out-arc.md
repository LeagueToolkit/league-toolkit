# ADR-0011: The object cache is an installed provider that hands out `Arc`

- **Status:** Accepted
- **Date:** 2026-08-30
- **Crates:** `ltk_meta`
- **Related:** PRD-002 (FR-10), #192, #209,
  `docs/design/bin-streaming.md` [section 4.4](../design/bin-streaming.md#s4.4)

## Context and problem statement

Repeated lookups into one file should not re-parse: the bin editor chases `ObjectLink`s around a
scene, and `ltk-manager` resolves the same objects across requests. Some form of per-handle caching
is wanted.

What forces a decision now rather than later is the **return type**. `object()` hands back owned
data; a cache that hands back a borrow would tie every cached value's lifetime to the handle and
make eviction a borrow-checker problem. Whatever the cache returns is public API from the first
release, and it is the one part of this that cannot be retrofitted - the implementation can trail,
the signature cannot.

## Decision drivers

- Eviction must never invalidate a value a caller is still holding.
- A handle with a cache installed must stay `Send`, for `ltk-manager`'s per-document workers.
- The eviction policy is the consumer's business, not the crate's.
- Do not special-case "caching disabled" into a second code path.

## Considered options

1. **`Option<Cache>` on the handle, returning `&BinObject<M>`** - borrow from the cache.
2. **A dyn-compatible provider trait, returning `Arc<BinObject<M>>`**, with a real no-op provider
   as the default.
3. **A policy enum plus a third type parameter on the handle** - the cache as a static choice.

## Decision

**Option 2. The handle always holds a `Box<dyn ObjectCache<M> + Send>`, `NoCache` by default, and
`cached_object()` returns `Arc<BinObject<M>>`.**

Only `cached_object()` consults it; the cursors and `object()` never do, so a sweep cannot evict
what a consumer is holding hot. [Section 4.4](../design/bin-streaming.md#s4.4) specifies the trait,
the shipped providers and the rest of the surface.

## Consequences

- **Positive:** a hit is an `Arc` clone. Callers keep values as long as they like, eviction
  invalidates nothing, and values cross threads. The provider *is* the policy, so bytes-bounded,
  TTL or shared-across-handles caches are the user's to write without the crate growing options.
- **Positive:** `NoCache` being a real provider rather than an absent one means there is one
  mechanism and no disabled-state branch to test.
- **Negative:** `Rc`-based providers are ruled out by the `Send` bound, deliberately. A
  single-threaded consumer pays for an atomic it does not need.
- **Negative:** one vtable call per lookup, and an `Arc` allocation per miss. Both are noise beside
  the parse being saved, but they are not free, and a consumer that wants zero overhead has to use
  the uncached path.
- **Revisit when:** a consumer needs cached borrowed views rather than owned objects. That is a
  different feature, not a change to this one.

## Pros and cons of the options

### Option 1: `Option<Cache>` returning a borrow

- Good: no allocation, no atomic, no vtable.
- Bad: every cached value borrows the handle, so holding one blocks the next lookup, and eviction
  becomes a lifetime problem rather than a policy one. The `Option` also splits every cached path
  into a present and an absent case.

### Option 3: policy enum plus a type parameter

- Good: static dispatch, no vtable call.
- Bad: a third parameter on a handle that already carries `R` and `M`, appearing in every signature
  that mentions the handle - and it fixes the policy set at the crate boundary, so a consumer with
  a byte-budget cache cannot have one.
