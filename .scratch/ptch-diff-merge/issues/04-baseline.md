---
issue: 222
title: "Baseline: the three-way check for rebasing a patch onto a new build"
labels: crate:ltk_meta, enhancement, format:bin, area:api
---

**Dropped in review (D20, ADR-0006). This issue can be closed.** Recorded here so the reasoning is not
re-derived later. Design: `docs/prd/001-ptch-property-patches.md` [section 6](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/prd/001-ptch-property-patches.md#s6) and `docs/adr/0006-no-schema-in-ltk-meta.md`.

The proposal was a `Baseline` type capturing the value every record of a `PTCH` was authored over,
stored beside the patch, so a later install could tell a stale record from a wanted one without
keeping the original base bin.

Two reasons it goes:

**It would cry wolf.** A mod is authoritative where it speaks. An author who set `Anchor = (0,1)`
wants `(0,1)` whatever Riot moved it to since, so "the base changed underneath this record" is
usually the mod working correctly. Reported per record, it would fire on most records of most mods
after most patches.

**The failure that does hurt is answerable without it.** What actually breaks a mod across builds
is a **type migration**: Riot changes a property's type in place, 337 times in three years and 327
in the single 16.17 `String` -> `File` patch. The client's tag rule is exact byte equality, and a
value whose tag does not match is discarded with no error and no log line. On one champion WAD
across that patch, 0 `File` values become 3,778 across 10 fields, led by
`StaticMaterialShaderSamplerDef.texturePath` and `AnimationResourceData.mAnimationFilePath` -
retexturing and custom animations. A mod predating the migration loses both, silently.

That is answerable from a per-build meta class dump alone, by comparing the registered tag for a
`(class, field)` between the build a mod was made for and the build it is being installed on. It
needs no old bin and nothing captured at authoring time. On the merge path it needs even less:
the two values differ in kind, so `Replaced::mismatched` (#220) already marks every one.

## What stays uncovered, and when to revisit

A record carrying a **composite** value overwrites fields its author never touched. Riot's own
corpus is 3,495 `Embed` and 2,885 `Pointer` records out of 23,047, so composite records are
ordinary rather than exotic, and the collateral belongs to the record language rather than to any
diff we write.

If that turns out to matter in practice, the answer is not a baseline over every record but one
over composite records only, reporting drift in the fields the author did not change. That is a
much smaller thing than what this issue proposed, and it is a companion to the parked authoring
flow (#221) rather than something to decide now.
