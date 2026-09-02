# PRD-001: PTCH patch support in `ltk_meta`

- **Status:** In progress - phases 1 and 2 implemented, 3 and 4 designed
- **Created:** 2026-08-31
- **Crates:** `ltk_meta`, `ltk_ritobin`, `ltk_file`
- **Tracking:** [#172](https://github.com/LeagueToolkit/league-toolkit/issues/172),
  [#173](https://github.com/LeagueToolkit/league-toolkit/issues/173),
  [#218](https://github.com/LeagueToolkit/league-toolkit/issues/218); tickets in
  `.scratch/ptch-diff-merge/issues/`
- **Spec:** `docs/design/ptch-property-patches.md`; `docs/design/value-walk.md` for the walk
  and `ValuePath`
- **Decisions:** ADR-0001 to ADR-0006, ADR-0012 to ADR-0014

## <a id="s1"></a>1. Problem

Two problems, one small and one large.

**The toolkit cannot read a patch bin.** A `PTCH` file is a patch applied over exactly one base
`PROP` bin: a set of object hashes to delete, whole objects to add, and a list of property-patch
records, each naming one object by hash, one property inside it by a typed path string, and a
value. Riot ships 238 of them - all UI scene variants - carrying 23,047 records against 582 whole
objects. When this work started, `ltk_meta` mis-read the header's count field, did not read the
records at all, and `Bin::to_writer` had a `todo!()` for override bins: every tool built on the
crate was blind to 23,047 authored edits, and none could produce one.

**A mod that ships a bin the game also has destroys what it did not carry forward.** This is the
defect `ltk-manager` ADR-0012 answers. The mod's chunk replaces the game's, so every object the
author did not copy is gone from what the client loads. In one measured specimen the loaded view
holds 847 objects where the game holds 1,473, and 1,151 `ResourceResolver` map keys go with them.
The severity is what forces the shape of the fix: a resolver miss can crash, whether it does
depends on the call site rather than on the key, and the call sites are compiled spell scripts
outside every bin. Nothing readable says which keys are the dangerous ones, so a repair that
restores most of them leaves an unknown subset of crashes standing. **The repair has to be total.**

## <a id="s2"></a>2. Objective

`ltk_meta` reads, writes and applies `PTCH` patches with the client's own semantics, and can layer
one bin over another so that a build can keep the game's content everywhere a mod is silent.
Success is a mod manager able to produce an overlay in which the specimen above loads 1,473
objects rather than 847, computed against whatever build is installed at the time.

## <a id="s3"></a>3. Consumers and stories

A consumer here is a crate, a tool, or a person building one.

1. As **`ltk-manager`**, I want to layer a mod's bin over the game's copy, so that a mod repairs
   what it dropped instead of deleting it (ADR-0012).
2. As **`ltk-manager`**, I want to know before I write anything whether a mod's patch still fits
   the installed build, so that I can report a stale mod instead of shipping a broken overlay.
3. As **a mod author**, I want to ship a `PTCH` rather than a whole bin, so that my mod says only
   what I changed and survives a game update that touches the rest of the file.
4. As **a mod build tool** (`league-mod`), I want to name a patch and its target declaratively, so
   that a patch reaches any bin rather than only the ones the UI scene manager registers.
5. As **`ltk_ritobin` and the LSP**, I want a patch bin to print and parse as text, so that a
   patch is editable by hand and diffable in review.
6. As **a scanning tool**, I want one entry point that reads whichever kind of bin a file is, so
   that I do not sniff magics myself.
7. As **`ltk-manager`'s problems pass**, I want one read-only traversal over every node of a bin,
   with an address for any node I report on, so that each health-check rule is a visitor rather
   than its own walker - the manager has two hand-written walkers today and is about to need a
   third that runs several rules over one pass.

## <a id="s4"></a>4. Requirements

### Functional

- **FR-1:** The crate SHALL read a `PTCH` container - delete list, whole objects, and the record
  list - and expose the records as typed values rather than bytes.
- **FR-2:** The crate SHALL write a patch bin back byte-identically for every file Riot ships.
- **FR-3:** A record's property path SHALL be parsed and validated when the file is read, not when
  it is resolved.
- **FR-4:** The crate SHALL resolve a path against a bin object and write a value at the position
  it names, applying the client's type rule.
- **FR-5:** The crate SHALL apply a whole patch over a base bin, and SHALL offer the same walk
  without mutation, so a caller can ask "does this still fit this build" before writing anything.
  Both SHALL report what was skipped and why, because the client's own apply is non-fatal per
  record.
- **FR-6:** `ltk_ritobin` SHALL print and parse a patch bin, byte-compatible with the existing
  ritobin output for every shipped file.
- **FR-7:** The crate SHALL merge one bin over another with the semantics ADR-0012 names: a plain
  value replaces, a map combines key by key, an object and an embedded struct combine field by
  field, and where the mod says nothing the game's content survives.
- **FR-8:** The crate SHALL report, per record and in file order, whether applying it replaced a
  value, created one, or did nothing, so that a caller holding a schema can drop records that say
  nothing.
- **FR-9:** The crate SHALL concatenate several patches aimed at one target and report every
  position two of them write.
- **FR-10:** The crate SHALL be able to express the difference between two bins as a patch, and
  SHALL report every difference the record language cannot carry. *(Parked - ADR-0004.)*
- **FR-11:** The crate SHALL offer one entry point that reads whichever kind of bin a file holds.
- **FR-12:** The crate SHALL walk every node of an object - the object and every nested struct
  and embed - in a fixed order, calling a visitor once per node, and SHALL let the visitor decline
  to enter any property so that nothing beneath it is visited. The same visitor SHALL run over an
  owned object and over a streamed object's buffered bytes, materialising nothing in the second
  case.
- **FR-13:** The crate SHALL address any position inside an object by hash and by position,
  carrying the class each field was read on, and SHALL render an address in a form that is stable
  across machines and name tables and in a best-effort readable form that says how much of it a
  name table could spell.

### Non-functional

- **Fidelity:** every shipped file round-trips byte-identically. This is the test that the format
  is understood rather than approximated.
- **No schema dependency:** none of the above may require Riot's meta class definitions. A
  consumer that has them can do more, and one that does not still gets FR-1 to FR-11 (ADR-0006).
- **No cloning on the caller's behalf:** applying a patch moves its objects and values into the
  base. A caller that wants to reuse a patch says so.

## <a id="s5"></a>5. Constraints from the game

Facts the design has no freedom about. Sources: the decompiled loader
([appendix A](../design/ptch-property-patches.md#appendix-a)), `PropertyPath.hpp`, and the
reversing notes in `league_structs` (`PTCH_PropertyPatches`, `BinFileCache_DataOverrides`,
`PropertyOverrideLoadable`).

**A patch is never a file's root data.** It is only ever loaded as an override of a named base
bin. The registration, however, is *data*: `PropertyOverrideLoadable` is a shipped bin class
holding a `{patch, target}` pair and an `active` byte, and the UI scene manager walks its pending
list on scene load and calls `BinFileCache_addDataOverride_byFile`. Three delivery routes follow:

1. **Replace a patch Riot already registers.** In the 16.13 dump the 220 distinct patch hashes
   named by `PropertyOverrideLoadable` objects are an exact set match with the 220 `PTCH` files in
   the game, so every shipped patch is reachable this way with no new plumbing.
2. **Author a new pair against a condition that already exists**, by hanging a
   `UiPropertyOverrideLoadable` off one of the 105 declared link properties (`FlippedOverride`,
   `MobileOverrideLoadable`, and the rest). Registration, priority, activation, teardown and live
   reload come free.
3. **Declare the target and let a build apply it** (`league-mod`
   [#191](https://github.com/LeagueToolkit/league-mod/issues/191)). Not a client mechanism, and
   the only route that generalises: any bin can be a target because nothing registers at runtime.

Constraints on routes 1 and 2:

- **A new *condition* cannot be authored in data.** Each is a hardcoded `setOverrideActive` call
  site, 87 of them, one per condition per view controller. Data only names which override an
  existing condition drives.
- **Validation is strict and failure is total.** `PTCH` version 1 around inner `PROP` version 3,
  though the base may be v2 or v3; a malformed patch fails the whole bin load, not just the patch.
- **Priority orders the registrations.** Records are sorted ascending by priority and the later
  one wins; shipped values are 0, 3, 7 and 8.
- **Registration precedes the parse.** An override only affects bins parsed after it is
  registered.
- **Both routes are scoped by the UI scene manager**, the only consumer of the file-to-file
  override API in the executable.

**A patch can be a wildcard**, applying to every bin the client parses rather than to one named
target, from two sources: the game server, whose `GameStartInfo` bin blobs are registered by
memory with a null target and torn down at match end; and a declaration whose **target hash is 0**,
which `BinFileCache_createEntry` falls through for every bin. The second is data, so it is the one
a mod can write. How far a UI-registered wildcard reaches is **not attested** - no shipped
`PropertyOverrideLoadable` has a zero target, and this has not been tested in game.

**Riot changes property types in place**, 337 times in three years and 327 in the single 16.17
`String` -> `File` patch. The client's tag rule is exact byte equality with no coercion, and a
value whose tag does not match is consumed and discarded with no error and no log line.

## <a id="s6"></a>6. Failure modes

What breaks a mod on a build its author never saw, ranked by how often it actually happens.

| Rank | Failure | What it looks like | What the design owes it |
| --- | --- | --- | --- |
| 1 | **Type migration.** Riot changes a property's type in place. | Nothing fails. The field is silently left at whatever the object's constructor put there. Measured on one champion WAD across 16.16 to 16.17: 0 `File` values become 3,778 across 10 fields, led by `texturePath` (1,826) and `mAnimationFilePath` (1,595) - retexturing and custom animations, the two commonest things a skin mod does. | Answerable from a meta class dump alone, with nothing captured at authoring time. On the merge path it needs even less: the values differ in kind, so the merge report already marks every one (FR-7). |
| 2 | **A moved or renamed property.** | The path stops resolving; the record is skipped and named. | Already covered by FR-5. |
| 3 | **A changed base value.** The base holds something different at a leaf than it did when the patch was authored. | The record overwrites it. | **Deliberately not chased.** A mod is authoritative where it speaks, so reporting this would fire on every record of every mod after every patch (ADR-0006). |

What remains uncovered is narrow and worth naming: a record carrying a **composite** value
overwrites fields its author never touched. Riot's own corpus is 3,495 `Embed` and 2,885 `Pointer`
records out of 23,047, so composite records are ordinary rather than exotic, and the collateral
belongs to the record language rather than to anything the toolkit writes.

So the guarantee a mod carries, stated as a user should hear it:

- **Authored on the build it is installed on:** exact.
- **Authored on an earlier build:** *structurally valid*, not *does what the author intended*. The
  property still exists at the same type; nothing says the base's value is the one the author was
  working from.

## <a id="s7"></a>7. Out of scope

- **A class schema inside `ltk_meta`.** The resolver works on the serialized value tree, not on
  Riot's meta classes. Reproducing the client's apply is in scope; judging a mod against Riot's
  meta classes is not (ADR-0006).
- **Capturing what a record was authored over** so a later build can detect drift under it
  (ADR-0006).
- **Registering a patch in a running client.**
- **A CLI.** A `ptch check` command belongs to whichever tool wants it.
- **Element-wise container merging.** A list has no key to combine by (ADR-0004).

## <a id="s8"></a>8. Acceptance

- [ ] **AC-1:** All 238 shipped patch files parse to the exact end of the chunk and write back
      byte-identically (FR-1, FR-2).
- [ ] **AC-2:** Every shipped record resolves against its base with the outcome the corpus scan
      predicts, and the skip counts match (FR-3, FR-4, FR-5).
- [ ] **AC-3:** Every shipped patch prints as text byte-compatibly with ritobin (FR-6).
- [ ] **AC-4:** Merging the ADR-0012 specimen over the game's copy yields 1,473 objects, keeps the
      mod's 4,788 bindings and its 84 added keys, and breaks no link the game leaves open (FR-7).
- [ ] **AC-5:** A per-record report agrees with the aggregate counts it is derived from, so the
      two surfaces cannot drift (FR-8).
- [ ] **AC-6:** Joining two patches that write the same position reports it, and applying the
      joined patch equals applying each in order (FR-9).
- [ ] **AC-7:** A walk over every object in an install visits each struct and embed with a
      non-zero class exactly once, in file order, and for every position in a fixture tree with a
      complete name table the address renders to a path that resolves back to the same value
      (FR-12, FR-13).
