---
issue: 223
title: "join: concatenate patch overrides and report collisions"
labels: crate:ltk_meta, enhancement, format:bin, area:api
---

Part of #218 (design: `docs/design/ptch-property-patches.md` [section 13](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/ptch-property-patches.md#s13)).
Several mods over one bin, and knowing where they meet before anything is applied.

Records are an ordered list, so overrides concatenate and apply in order, last writer winning. What
a build wants first is to know when that matters - a collision reported at build time is worth more
than one discovered in game.

## Proposed surface

```rust
/// Two overrides that write the same place.
///
/// `overrides` is always a pair of positions in the order `join` was given, earlier first.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Collision {
    /// Two records address the same object and path.
    Record { object_hash: BinHash, path: PropertyPath, overrides: (usize, usize) },
    /// One override replaces an object whole; another patches inside it.
    Object { object_hash: BinHash, overrides: (usize, usize) },
    /// One override deletes an object another writes to.
    Deleted { object_hash: BinHash, overrides: (usize, usize) },
}

/// Concatenates the overrides in order and reports every place two of them meet.
pub fn join<M>(overrides: impl IntoIterator<Item = BinOverride<M>>)
    -> (BinOverride<M>, Vec<Collision>);
```

## Rationale

**D23 `join` reports, `apply` resolves.** Collisions are data for the caller; last-writer-wins is
what applying in order already does. Which override should win is policy, and a manager that knows
the user's load order has more to go on than this crate does.

**D29 a `BinOverride` is not a layer (ADR-0001).** The client-side reversing notes use "layer" throughout,
where it means the client's cache entry attaching a `PTCH` to a base bin rather than the file
itself. That is a different thing from this crate's type, so the word stays out of the API. It
remains in use as a verb, because ADR-0012 defines the merge as layering a mod's content over the
game's copy. `override` alone is reserved in Rust; the plural is not.

Path comparison for `Collision::Record` is textual, which is what `PropertyPath`'s `PartialEq`
already gives. Two paths that select the same property through different casing therefore do not
collide; comparing `Segment::name_hash` values would catch those, and is the follow-up if it ever
matters in practice.

- [ ] Two overrides setting the same object and path report one `Collision::Record` naming both
      positions, and the later record wins on apply
- [ ] An override carrying a whole object collides with any other's record inside that object
- [ ] An override deleting an object collides with another's records against it
- [ ] Overrides that touch disjoint objects report nothing
- [ ] The joined override applies to the same result as applying each one in order
