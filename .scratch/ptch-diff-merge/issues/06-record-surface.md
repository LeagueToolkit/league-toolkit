---
issue: 239
title: "Per-record apply outcomes and filter"
labels: crate:ltk_meta, enhancement, format:bin, area:api
---

Part of #218 (design: `docs/design/ptch-property-patches.md` [section 14](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/ptch-property-patches.md#s14); requirement PRD-001 FR-8).
What one record of a patch did, or would do, and a filter that judges each record against a base.
`check` reports aggregate counts, and a tool dropping the records that say nothing needs the
answer per record.

## Proposed surface

```rust
/// What one record did, or would do. Reported per record, in file order, so a caller can
/// decide what to keep without walking the base itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordOutcome {
    /// The leaf existed and the value replaced it.
    Replaced,
    /// The leaf did not exist and was created. The case a schema-holding caller can strip.
    Inserted,
    /// The record did not apply. `ApplyReport::skipped` says why.
    Skipped,
}

impl ApplyReport {
    /// Per record, in file order.
    pub fn outcomes(&self) -> &[RecordOutcome];
}

impl<M> BinOverride<M> {
    /// Drops records `keep` rejects, judging each against `base` the way `check` does.
    ///
    /// The predicate sees the record and what applying it would do, which is everything a
    /// caller needs to consult a schema and decide.
    pub fn retain_with(&mut self, base: &Bin<M>,
        keep: impl FnMut(&PropertyPatch<M>, RecordOutcome) -> bool);
}
```

## Rationale

**D25, D26 (ADR-0006): the surface, not a `strip_noops` in the crate.** Reproducing the client's
apply is `ltk_meta`'s work; judging a mod against Riot's meta classes is not. Stripping needs the
meta class default for a `(class, field)`, which lives in the per-build dump, and runs outside as
a post-pass over this surface.

Only the insert case needs a schema. A record whose leaf the base does not serialize says nothing
exactly when its value equals the meta class default. A record whose value equals what the base
already serializes is one `Bin::diff` never writes. A record setting the meta class default over a
base that serializes something else is not a no-op, and stripping it reverts the mod.

Depends on nothing in #218 and can land in any order.

- [ ] `outcomes()` has one entry per record, in file order, agreeing with the aggregate counts
      `ApplyReport` already reports
- [ ] The corpus test asserts `outcomes()` against the counts in [appendix B](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/ptch-property-patches.md#appendix-b), and the two
      surfaces cannot drift
- [ ] `retain_with` judges each record against the base exactly as `check` does, including the
      insert case
- [ ] `retain_with` does not renumber or reorder the records it keeps
- [ ] Dropping a record that a later record's path depends on is the caller's problem, and is
      documented as such
