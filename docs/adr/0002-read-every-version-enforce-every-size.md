# ADR-0002: The reader accepts every version it knows and enforces the sizes the client ignores

- **Status:** Accepted
- **Date:** 2026-08-22
- **Crates:** `ltk_meta`
- **Related:** PRD-001 (FR-1, FR-2), #172, `docs/design/ptch-property-patches.md`
  [section 4](../design/ptch-property-patches.md#s4) and
  [section 7](../design/ptch-property-patches.md#s7)

## Context and problem statement

The client's reader and the toolkit's have different jobs, and copying the client exactly would do
the toolkit's job badly in both directions.

The client is strict where it does not need to be: it gates the outer version at 1 and the inner
at 3, and a file outside that fails the whole bin load. It is lax where the toolkit cannot afford
to be: `payloadSize` is written into every record and never checked, so a file whose records
disagree with their own size fields loads happily and reads whatever follows.

The toolkit reads files from every era, including ones the current client would refuse, and its
failure mode has to be an error rather than a plausible-looking mis-parse.

## Decision drivers

- Read every version the format has had; a tool that cannot open an old file is not a toolkit.
- Never mis-read. A size that disagrees with its own payload means the parse is already lost.
- Keep "this file parses" and "this file loads in the client" separate questions with separate
  answers.

## Considered options

1. **Mirror the client** - same version gates, same ignored sizes.
2. **Accept every known version, enforce every size** - diverge from the client in both
   directions, deliberately.
3. **Accept everything and warn** - parse what can be parsed, collect diagnostics.

## Decision

**Option 2.**

- `Bin::from_reader` accepts `PROP` versions 1 to 3. `BinOverride::from_reader` accepts inner
  versions 1 to 3, reading the record list only for version 3 - the gate LtMAO uses too. Both
  writers emit 3, and no version field is exposed or settable.
- A `payloadSize` that disagrees with the body read under `ltk_io_ext::measure` is `InvalidSize`,
  consistent with how objects, structs and maps are already read.
- A non-zero `dependencyCount` is `OverrideDependencies` and dependencies are not representable at
  all: a patch that declares any cannot load in any client, so there is nothing to round-trip.
- Object kinds get the shared reader's legacy-numbering retry. Record kinds do not: records exist
  only in inner version 3, which postdates the renumbering, so a legacy record list cannot exist
  and one that somehow did would fail with `InvalidPropertyTypePrimitive` rather than be misread.

The client's own gates are documented in the design doc's wire-format section rather than enforced
on read.

## Consequences

- **Positive:** old files stay readable, a corrupt file is caught at the record that is wrong
  rather than several records later, and the writer cannot emit a version the client refuses.
- **Negative:** "this crate parsed it" is not "the client will load it". The two questions are
  answered by different surfaces - the reader for the first, `check` for the second - and a
  consumer that conflates them will ship a patch the game rejects.
- **Revisit when:** an inner version 4 appears. The gate is one constant and the record list's
  version condition; nothing else in the design assumes 3.

## Pros and cons of the options

### Option 1: mirror the client

- Good: one rule to explain; a file that parses is a file that loads.
- Bad: closes the door on every historical file, and inherits a size field nobody checks.

### Option 3: accept everything and warn

- Good: maximally permissive for a scanning tool.
- Bad: a warning nobody reads is a mis-parse with extra steps, and the diagnostics channel does
  not exist in this crate.
