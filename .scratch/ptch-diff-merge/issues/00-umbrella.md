---
issue: 218
title: "Bin diff and merge"
labels: crate:ltk_meta, enhancement, format:bin, area:api
---

Layering one bin over another, and saying the difference between two bins as a `PTCH`.

## Documents

| What | Where |
| --- | --- |
| Why this exists, who asks for it, requirements, delivery routes, failure modes | `docs/prd/001-ptch-property-patches.md` |
| API surface, wire format, merge walk, testing | `docs/design/ptch-property-patches.md` [section 10](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/ptch-property-patches.md#s10) to [section 14](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/ptch-property-patches.md#s14) |
| Merge and diff are separate operations | `docs/adr/0004-merge-and-diff-are-separate-operations.md` |
| `ValuePath` and the walk that produces one | `docs/design/value-walk.md` |
| `ValuePath` addresses by hash | `docs/adr/0005-value-path-addresses-by-hash.md` |
| Path class context | `docs/adr/0012-path-class-context.md` |
| Single-visitor walk | `docs/adr/0013-single-visitor-walk.md` |
| Tree traits under the walk | `docs/adr/0014-tree-traits-under-the-walk.md` |
| No schema in `ltk_meta` | `docs/adr/0006-no-schema-in-ltk-meta.md` |

The tickets below render the spec's API-surface sections. Every rule the spec settles is a `Dn` row
in its [section 17](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/ptch-property-patches.md#s17), each naming the ADR that argues it where one does.

## What this is for

The consumer is `ltk-manager`. Its ADR-0012 ("The overlay merges a mod over the game's copy",
accepted 2026-08-31) decides that the overlay build layers a mod's content over the game's copy of
a chunk instead of letting the chunk replace it, and names PTCH record semantics as the merge it
means: a plain value replaces, a map combines key by key, an object and an embedded struct combine
field by field, and where the mod says nothing the game's content survives. The defect it answers,
measured on one specimen, is a mod bin holding 847 objects where the game holds 1,473, taking 1,151
`ResourceResolver` map keys with it. A resolver miss can crash, and which keys are dangerous is
decided by compiled spell scripts outside every bin, so the repair has to be total.

The manager takes a mod as an **edited `PROP` bin** (diffed against the game's copy at build time
and layered) or as a **`PTCH`** (applied with `BinOverride::apply`, which phase 2 shipped). A patch
is also a delivery format in its own right, by three routes - two the client's own and one
declarative (`league-mod` #191) - which PRD-001 [section 5](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/prd/001-ptch-property-patches.md#s5) sets out along with what the client
constrains. Route 3 is served almost entirely by phase 2 and wants two things already scheduled
here: `ApplyReport::outcomes` (#221's ready half) and `join` (#223).

## Children

- [ ] #219 — `ValuePath`: addressing a position in a bin by hash (goes first; #220 and #225 rest on it; it carries a class context beside its steps, ADR-0012)
- [ ] #220 — `Bin::merge`: layer one bin over another (what the overlay build needs; the only one
      of these with a consumer waiting)
- [ ] #221 — the per-record report and filter (`RecordOutcome`, `ApplyReport::outcomes`,
      `BinOverride::retain_with`), plus `Bin::diff`, which is designed and **parked** (D27)
- [ ] #223 — `join`: concatenate patch overrides and report collisions
- #222 — `Baseline`: **dropped** (D20). The failure it chased is answerable from a meta class
      dump with nothing captured; see the issue for the reasoning and the narrow case left open.

#221's record surface and #223 depend on nothing here and can land in any order.

## Why merge and diff are two operations

The tempting shape is one operation: diff the two bins into a `BinOverride`, then `apply` it, so
both input paths converge on the record language. It does not hold. **A record cannot insert a map
entry** - `patch_in` creates a leaf only when the last segment names it outright and its parent is
an object, a Struct or an Embed, and a `{key}` subscript needs an entry to subscript. A mod adding
84 map keys has no record set that says so; the closest expressible record carries the whole map,
which is the wholesale replacement ADR-0012 exists to stop.

So `merge` applies in process with no serialization constraint, and `diff` renders as much of the
same walk as records can carry, reporting every place it could not. The invariant tying them is in
[section 12](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/ptch-property-patches.md#s12):

```text
base.diff(edited).apply(base)  ==  base.merge(edited)      when DiffReport::lifted is empty
```

## The boundary: no schema in `ltk_meta` (D25, ADR-0006)

`lol-meta-classes` dumps every build's meta classes, which answers questions this design
previously called unanswerable. The line drawn is **reproducing the client's apply is `ltk_meta`'s
work, judging a mod against Riot's meta classes is not**, so stripping records that say nothing
runs outside the crate as a post-pass - which is what #221's record surface exists to serve. D8
and D11 stand as taken rather than being closed with the base chain now in reach. ADR-0006 has the
argument, including which of the two no-op cases actually needs a schema and why stripping the
other would silently revert a mod.

## What this does and does not buy

A patch names positions by hash, and hashes are stable across builds, so a patch that still fits
still applies. What actually breaks a mod across builds is a **type migration**, which PRD-001
[section 6](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/prd/001-ptch-property-patches.md#s6) ranks and measures: on one champion WAD across 16.17, 0 `File` values become 3,778, led
by the texture-path and animation-path fields. That is answerable from the dump alone, and on the
merge path from `Replaced::mismatched` (#220) without even that.

What no operation here can tell is "the author wanted this exact value" from "the author moved the
value that was there"; #222 records why chasing that was dropped. Merging rather than replacing
bounds the damage a stale mod does to what its bin actually contains; it does not make a stale mod
current.
