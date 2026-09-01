---
issue: 221
title: "Bin::diff: two bins into a BinOverride"
labels: crate:ltk_meta, enhancement, format:bin, area:api
---

Part of #218 (design: `docs/design/ptch-property-patches.md` [section 12](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/ptch-property-patches.md#s12) and [section 14](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/ptch-property-patches.md#s14)). Two pieces
that arrived together and separated during review:

- **Ready: the per-record surface.** `check` reports aggregate counts, so nothing outside the
  crate can tell which record did what. A tool that wants to drop records saying nothing needs
  that per record.
- **Parked: `Bin::diff`.** Designed in 16.5, not scheduled (D27, ADR-0004). Its only consumer would be an
  authoring flow turning a modder's edited bin into a `PTCH` on the install it was made on, and
  no such flow exists; the manager's overlay build needs `merge`, not `diff`. Same treatment as
  #217: written down so the shape is settled, built when something asks for it.

## Proposed surface: the per-record report and filter

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

**Why this shape, and not a `strip_noops` inside the crate (D25, D26; ADR-0006).** Reproducing the client's
apply is `ltk_meta`'s work; judging a mod against Riot's meta classes is not. Stripping needs the
meta class default for a `(class, field)`, which lives in the per-build dump, so it runs outside
as a post-pass and this is the surface it needs.

Only one of the two no-op cases needs a schema at all, and it is the insert case: a record whose
leaf the base does not serialize is a no-op exactly when its value equals the meta class default.
The other case - a record whose value equals what the base already serializes - is never emitted
by a correct diff, so no rule is needed for it. A record setting the meta class default over a
base that serializes something else is **not** a no-op, and stripping it would silently revert the
mod.

## Parked surface: `Bin::diff`

[section 12](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/ptch-property-patches.md#s12) carries the full signatures (`DiffOptions`, `Lift`, `DiffReport`, `diff`,
`diff_with`), the escalation ladder, and the invariant tying a diff to a merge:

```text
base.diff(edited).apply(base)  ==  base.merge(edited)      when DiffReport::lifted is empty
```

Nothing there changes; it is simply not scheduled. Its acceptance checklist stays in the design
doc rather than here until it is picked up.

Blocked by #219 (the parked half only; the record surface depends on nothing)

- [ ] `outcomes()` has one entry per record, in file order, agreeing with the aggregate counts
      `ApplyReport` already reports
- [ ] The corpus test asserts `outcomes()` against the counts in [appendix B](https://github.com/LeagueToolkit/league-toolkit/blob/main/docs/design/ptch-property-patches.md#appendix-b), so the two
      surfaces cannot drift
- [ ] `retain_with` judges each record against the base exactly as `check` does, including the
      insert case
- [ ] `retain_with` does not renumber or reorder the records it keeps
- [ ] Dropping a record that a later record's path depends on is the caller's problem, and is
      documented as such
