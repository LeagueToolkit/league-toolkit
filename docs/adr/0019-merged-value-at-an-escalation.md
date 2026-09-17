# ADR-0019: Merged value at an escalation

- **Status:** Accepted
- **Date:** 2026-09-17
- **Crates:** `ltk_meta`
- **Related:** PRD-001 (FR-7, FR-10), ADR-0004, #221,
  `docs/design/ptch-property-patches.md` [section 12](../design/ptch-property-patches.md#s12)

## Context and problem statement

`Bin::diff` writes a difference as a record at the position it is found. Three things stop a
record there. No record inserts a map entry. No record changes the shape of a value: the patch
type rule skips it ([section 9.3](../design/ptch-property-patches.md#s9.3)). No record addresses a
position whose path has an unnamed field. At each of these, the diff escalates: it writes one
record at an ancestor, or takes the whole object, and reports a `Lift`.

The escalated record carries a whole value, and the diff has two whole values to hand at that
position: the edit's, and the base's with the edit merged over it. The two differ exactly where
the base holds something the edit omits: a map key the mod dropped, a property the mod's copy of a
struct lacks.

`ltk-manager` ADR-0012 exists to keep those. Its measured specimen is a mod bin holding 847
objects where the game holds 1,473, dropping 1,151 `ResourceResolver` keys, and a resolver miss can
crash. `league-mod` converts a mod's replaced bins into a `.ptch` and into game-data declarations
with this diff (its `docs/research/bin-diff-to-declarations.md`), and checks each conversion
against `Bin::merge` on the base it was made from. A `.ptch` applies to the install the conversion
ran on and, after a patch, to a later one.

## Decision drivers

- A converted mod applied to the base it was made from loses nothing a merge keeps (FR-7).
- The difference between the patch and the merge is testable as one invariant (FR-10).
- A lift tells the author what the patch carries beyond the change.

## Considered options

1. **The edit's value** - the escalated record carries what the edit holds at the ancestor.
2. **The merged value** - the escalated record carries the base's value with the edit merged over
   it.
3. **No record** - the diff reports the lift and writes nothing for that difference.

## Decision

**Option 2. An escalated record, and a whole object taken at the root, carry the base's value
with the edit merged over it.** Every patch `diff` produces, applied to the base it was made from,
equals `Bin::merge` of the same pair, whatever it lifted. The escalation and the invariant are
`docs/design/ptch-property-patches.md` [section 12](../design/ptch-property-patches.md#s12).

A `Lift` marks a record that carries more than the difference. Applied to another base, that
record overwrites whatever the other base holds inside its value.

## Consequences

- **Positive:** a conversion verifies against `Bin::merge` with no lift-dependent exception. On the
  install a mod was converted on, the `.ptch` keeps every map key and property the game holds and
  the mod omits.
- **Negative:** an escalated record pins the base's own values beside the mod's. After a game
  patch changes one of those values, the record writes the old one back. A lift is the only mark
  of it, and the record itself does not say which of its values came from the mod.
- **Revisit when:** the record language gains a map-entry insert. The largest class of escalation
  then becomes a precise record.

## Pros and cons of the options

### Option 1: the edit's value

- Good: the record holds only what the mod shipped, and nothing of the install it was converted
  on.
- Bad: applied to its own base, a lifted map record deletes every key the mod omitted. That is the
  crash ADR-0012 of `ltk-manager` measures, reproduced by the tool meant to convert the mod away
  from it. The invariant holds only where nothing lifted, and a conversion's verifier needs a
  per-lift exception.

### Option 3: no record

- Good: the patch never overwrites anything the mod did not name.
- Bad: the patch drops the mod's change. A map insert, the most common escalation, vanishes from
  the export, and the merge invariant fails on every lifted position.
