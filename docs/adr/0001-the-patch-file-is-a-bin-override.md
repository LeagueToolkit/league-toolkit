# ADR-0001: The patch file is a `BinOverride` and the records inside it are patches

- **Status:** Accepted
- **Date:** 2026-08-22
- **Crates:** `ltk_meta`, `ltk_file`, `ltk_ritobin`
- **Related:** PRD-001, #172, `docs/design/ptch-property-patches.md`
  [section 2](../design/ptch-property-patches.md#s2),
  [section 5.1](../design/ptch-property-patches.md#s5.1) and
  [section 5.2](../design/ptch-property-patches.md#s5.2)

## Context and problem statement

The file has three names already in circulation, and the crate has to pick one for the type and
one for the records:

- **Riot's loader calls it a data override.** `BinFileCache_addDataOverride`,
  `PropertyOverrideLoadable`, the `cache->overrides` list.
- **The magic and every community tool call it a patch.** `PTCH`; LtMAO's `is_patch` and
  `patches`; ritobin's `patches` root.
- **The client-side reversing notes call it a layer.** `layerObjectCounts`, "a patch layer's reach
  is exactly one bin file". There the word means the client's *cache entry* attaching a patch to a
  base bin, which is a different thing from the file.

`ltk_file` already committed to one of them: `LeagueFileKind::PropertyBinOverride`. And Rust
reserves `override`, so it cannot name a module or a method without `r#`.

## Decision drivers

- Use the game's own vocabulary where the game has one.
- Use the community's where the game is silent, so that a reader of ritobin output recognises it.
- Do not invent a third noun for a thing that already has two.
- Compile without raw identifiers.

## Considered options

1. **`BinOverride` + `PropertyPatch`** - Riot's noun for the file, everyone's noun for the record.
2. **`BinPatch` + `PropertyPatch`** - the community's noun throughout.
3. **`BinOverride` + `PropertyOverride`** - Riot's noun throughout.

## Decision

**Option 1. The file type is `BinOverride`, its records are `PropertyPatch`, the per-property verb
is `patch()`, and the private module is `data_override`.**

What this settles for the rest of the vocabulary - that `layer` stays a verb and never becomes a
third noun for this file, and that a position among several overrides is spelled `overrides` - is
defined with the rest of the domain terms in the spec,
[section 2](../design/ptch-property-patches.md#s2).

## Consequences

- **Positive:** every name traces to a source rather than to taste, and `ltk_file` already agrees
  with it. A reader coming from the decompile finds the file where they expect it; a reader coming
  from ritobin finds the records where they expect them.
- **Negative:** the file and its records use different nouns, which needs one sentence of
  explanation the first time; [section 2](../design/ptch-property-patches.md#s2) carries it.
- **Revisit when:** never, realistically. This is public API in a released crate.

## Pros and cons of the options

### Option 1: `BinOverride` + `PropertyPatch`

- Good: matches `ltk_file`, the decompile, ritobin and LtMAO simultaneously.
- Bad: two nouns for one file's contents.

### Option 2: `BinPatch` throughout

- Good: one noun; matches the magic.
- Bad: contradicts `ltk_file`'s existing `PropertyBinOverride`, and loses the connection to the
  loader function names a reverser is reading from.

### Option 3: `BinOverride` + `PropertyOverride`

- Good: one noun, Riot's.
- Bad: nobody outside the decompile calls a record an override, and `override` collides with the
  keyword at every turn.
