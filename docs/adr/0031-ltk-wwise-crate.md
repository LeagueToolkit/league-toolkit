# ADR-0031: `ltk_wwise` crate

- **Status:** Proposed
- **Date:** 2026-10-28
- **Crates:** `ltk_wwise`, `ltk_hash`, `league-toolkit`
- **Related:** #300, `docs/design/wwise-audio.md` [section 1](../design/wwise-audio.md#s1) and
  [section 4](../design/wwise-audio.md#s4)

## Context and problem statement

The client plays all sound through Audiokinetic Wwise. A `BankUnit` in a bin names three kinds of
audio file: a media bank (`.bnk` with `DIDX` and `DATA`), an event bank (`.bnk` with `HIRC`) and a
package (`.wpk`). `ltk_file` identifies both extensions. No crate of the workspace reads or writes
either format. LTK Manager and league-mod have no way to list, replace or validate the audio of a
mod.

The bank format belongs to Audiokinetic. The package format belongs to Riot: the magic `r3d2`, an
offset table and UTF-16 names. The two formats share no header, no framing and no code. A package
carries WEM files. A media bank carries WEM files. A `BankUnit` loads a media bank, an event bank
and a package together.

Wwise names hash with FNV-1 32-bit. No crate of the workspace implements FNV-1.

## Decision drivers

- Each format parser of the workspace is one crate that compiles without the umbrella crate.
- A consumer that replaces a sound edits a package or a media bank. A consumer that finds the WEM
  of an event reads an event bank, its media bank and its package.
- The bank format has a public owner and a version history. The package format has neither.

## Considered options

1. **One crate `ltk_wwise`** with a module for packages and a module for banks. The hash goes in
   `ltk_hash`.
2. **Two crates `ltk_wpk` and `ltk_bnk`**, one per extension. The hash goes in `ltk_hash`.
3. **A module of `ltk_wad`**, next to the WAD reader that serves the package substream.

## Decision

**Option 1. `ltk_wwise` contains the package format, the bank format, the hierarchy layouts and
the version upgrade. `ltk_hash` contains FNV-1 as `fnv1`. `league-toolkit` re-exports the crate
with the `wwise` feature. The feature is off by default.**

The crate depends on `ltk_hash`, `byteorder` and `thiserror`. All three are workspace dependencies.
`ltk_wad` is a development dependency of the corpus tests. `docs/design/wwise-audio.md`
[section 4](../design/wwise-audio.md#s4) specifies the surface.

## Consequences

- **Positive:** a consumer depends on one crate for every file a `BankUnit` names. The event to
  media query spans banks and packages inside one crate. `ltk_hash` holds FNV-1 next to FNV-1a.
- **Negative:** a consumer that reads only packages compiles the bank layouts. The crate name is
  the vendor of the bank format. The package format is Riot's.
- **Revisit when:** the hierarchy layouts grow large enough that a package-only consumer pays a
  measurable build cost.

## Pros and cons of the options

### Option 2: `ltk_wpk` and `ltk_bnk`

- Good: each crate has the name of one extension. A package-only consumer compiles no bank code.
- Bad: the event to media query reads both formats. It needs a third crate or a dependency of
  `ltk_bnk` on `ltk_wpk`. The workspace gets two crates to version and publish for one feature.

### Option 3: a module of `ltk_wad`

- Good: the workspace gets no new crate. The client opens a package through a WAD substream.
- Bad: every `ltk_wad` consumer compiles an audio format that it does not read. A bank is not a
  WAD concept. The client loads a bank and a WAD chunk through different paths.
