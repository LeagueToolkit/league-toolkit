# ADR-0010: The metadata parameter lives on the handle, not on the methods

- **Status:** Accepted
- **Date:** 2026-08-30
- **Crates:** `ltk_meta`
- **Related:** PRD-002, #192, #207,
  `docs/design/bin-streaming.md` [section 4](../design/bin-streaming.md#s4)

## Context and problem statement

Every value-carrying type in `ltk_meta` is generic over a property-metadata parameter `M`, default
`NoMeta`, so `ltk_ritobin` can hang spans off a tree the rest of the crate reads without them. The
streaming types have to carry it too, and there are two places to put it: on the handle
(`BinStream<R, M = NoMeta>`) or on the value-producing methods (`read::<M>()`, `value::<M>()`,
`into_bin::<M>()`).

The first draft put it on the methods, before `ltk_meta::concrete` existed. `concrete` is a module
of `M = NoMeta` aliases that exists for a reason its own docs state: **Rust applies a type
parameter's default in type position but never in expression position.** A generic name in a `let`
needs an annotation or a turbofish; an alias that pins the parameter in type position removes both.

That fact decides the question, because a method-level `M` is exactly the position where no default
applies. Every `into_bin()`, `read()`, `value()` and `property()` call site would need `::<NoMeta>`
or an annotated binding, forever, in every consumer that never wanted metadata at all.

## Decision drivers

- The common case - a consumer with no metadata - should need no type annotations anywhere.
- Match how the crate already solved this for the eager types, rather than inventing a second shape.
- `concrete` can only pin type-position generics, so whatever is chosen has to be reachable by it.

## Considered options

1. **Method-level `M`** - `BinStream<R>` with `read::<M>()` and friends.
2. **Handle-level `M`** - `BinStream<R, M = NoMeta>`, with `concrete` growing stream aliases.

## Decision

**Option 2. `M` sits on the handle, pinned once at the `mount` call through a `concrete` alias,
after which it disappears from every downstream signature.**

`concrete` grows `BinStream`, `BinOverrideStream` and `BinFileStream` aliases. The views carry `M`
as a phantom so the owned-decode escape hatches infer without a turbofish, while the borrowed data
itself stays metadata-free. [Section 4](../design/bin-streaming.md#s4) specifies the surface.

## Consequences

- **Positive:** a consumer writes `concrete::BinStream::mount(file)?` and never spells `M` again.
  The streaming types read the same way the eager ones do.
- **Negative:** one handle is pinned to one `M` for its life. A consumer wanting both a
  span-carrying and a plain read of the same file mounts twice. Nothing in the workspace wants that,
  and the alternative charged every other consumer for it.
- **Negative:** `M` appears as a phantom on six view types, which means their `Clone`, `Copy` and
  `Debug` impls are written by hand - a derive would demand `M: Copy` for a field holding nothing.
- **Revisit when:** never, realistically. This is the crate's established shape and it is public
  API.

## Pros and cons of the options

### Option 1: method-level `M`

- Good: one handle can produce trees with different metadata; the parameter appears only where it
  is actually used.
- Bad: no default ever applies, so every value-producing call site in every consumer needs a
  turbofish or an annotation - and `concrete`, the crate's own answer to this problem, cannot reach
  a method-position generic to fix it.
