# ADR-0024: `ltk_ini` crate

- **Status:** Proposed
- **Date:** 2026-10-12
- **Crates:** `ltk_ini`, `ltk_hash`, `league-toolkit`
- **Related:** #119, #121, #122, `docs/design/inibin.md` [section 1](../design/inibin.md#s1) and
  [section 5](../design/inibin.md#s5)

## Context and problem statement

The game ships binary ini files with the extensions `.inibin` and `.troybin`. `.cfgbin` is a
third extension of the same format. The 16.20 client ships 1,589 of these files and reads them
with the functions in `docs/design/inibin.md` [appendix A](../design/inibin.md#appendix-a). The
C# LeagueToolkit reads and writes the format. No crate of this workspace reads or writes it. A
mod that changes a legacy particle system or a bot spell list must edit one of these files.

The key hash of the format is SDBM with the multiplier 65599. No crate of the workspace
implements that hash.

## Decision drivers

- Each format parser of the workspace is one crate that compiles without the umbrella crate.
- The format shares no magic, type system or hash function with property bins.
- The ini text files of the game are in the same format family. The crate name must not exclude
  them.

## Considered options

1. **A new crate `ltk_ini`**, with the hash in `ltk_hash`.
2. **A new crate `ltk_inibin`**, which is the name in #119, #121 and #122.
3. **A module of `ltk_meta`**, next to the property bin format that replaced this format.

## Decision

**Option 1. `ltk_ini` contains the binary ini format. `ltk_hash` contains the hash as `sdbm` and
the key type as `IniHash`. `league-toolkit` re-exports the crate with the `ini` feature. The
feature is off by default.**

The crate depends on `ltk_hash`, `glam`, `byteorder` and `thiserror`. All four are workspace
dependencies. The file type has the name of the format: `Inibin`. `docs/design/inibin.md`
[section 6](../design/inibin.md#s6) specifies the surface.

## Consequences

- **Positive:** a consumer that edits a troybin depends on one small crate. `ltk_hash` contains
  the hash together with the other game hashes. `IniHash` has the same shape as `BinHash` and
  `WadHash`.
- **Negative:** the workspace has one more crate to version and publish. The crate name suggests
  ini text support. The crate reads and writes only the binary format.
- **Revisit when:** the crate has the ini text format and a name table, and the name table is
  large enough to need a separate crate.

## Pros and cons of the options

### Option 2: `ltk_inibin`

- Good: the name is one of the file extensions. The issue and both pull requests use the name.
- Bad: the 16.20 client ships about three `.troybin` files for each `.inibin` file. A reader of
  ini text does not match a crate named `ltk_inibin`.

### Option 3: a module of `ltk_meta`

- Good: the workspace gets no new crate. Both formats store game properties under hashed names.
- Bad: every `ltk_meta` consumer compiles a format that it does not read. The two formats share
  no type. `ltk_meta` depends on `miette`, `indexmap` and `enum_dispatch`, and this format needs
  none of them.
