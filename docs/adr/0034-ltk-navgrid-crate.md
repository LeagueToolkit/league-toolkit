# ADR-0034: `ltk_navgrid` crate

- **Status:** Proposed
- **Date:** 2026-11-03
- **Crates:** `ltk_navgrid`, `league-toolkit`
- **Related:** `crates/ltk_navgrid/src/lib.rs`

## Context and problem statement

A map names its navigation grid in its `MapNavGrid` data. The client reads three files for it:
the grid (`.aimesh_ngrid`), flag overlays (`.ngrid_overlay`) and region tag overlays
(`.rg_overlay`). The grid holds per-cell heights, terrain flags, region tags, a sampled height
grid and a fixed-size hint grid. The overlays hold rectangles of flag words and of region tags.
No crate of the workspace reads or writes any of the three formats.

Every grid that ships is version 7. The client reads the version byte and parses the file with
the version 7 layout. Version 6 differs from version 7 in the flag and region tag arrays and in
the region tag table. A file of version 2 to 5 has a 60-byte cell record whose field widths are
not attested. No file of version 2 to 6 ships.

The three formats share the grid coordinates, the flag word and the region tag word. The map
geometry format (`.mapgeo`) shares none of them.

## Decision drivers

- Each format parser of the workspace is one crate that compiles without the umbrella crate.
- A consumer that edits the walkable area of a map writes a grid and its overlays together.
- A file that the caller does not edit writes back byte for byte.

## Considered options

1. **One crate `ltk_navgrid`** with the grid and both overlays.
2. **A module of `ltk_mapgeo`**, next to the other per-map format.
3. **A read-only parser** in a new crate, with no writer.

## Decision

**Option 1. `ltk_navgrid` reads and writes the grid and both overlays. `league-toolkit`
re-exports the crate with the `navgrid` feature. The feature is off by default.**

- `NavGrid::from_reader` reads versions 6 and 7. Every other version is
  `ParseError::UnsupportedVersion`. `NavGrid::to_writer` writes version 7.
- `CellFlags` is a `bitflags` `u16` that names the terrain tags and keeps every undeclared bit.
- The hint grid is a byte slice. Its record layout is not attested.
- A count that a file declares reserves memory through `ltk_io_ext`. A count larger than the
  stream fails at the end of the stream.

The crate depends on `ltk_io_ext`, `byteorder`, `bitflags`, `thiserror` and `glam`. All are
workspace dependencies.

## Consequences

- **Positive:** a consumer depends on one crate for every file that `MapNavGrid` names. Every
  shipped grid and overlay round-trips byte for byte.
- **Negative:** the crate rejects versions 2 to 5, which the client parses with the version 7
  layout. A version 6 grid writes back as version 7. The hint grid has no typed access.
- **Revisit when:** a file of version 2 to 5 is found, or the hint grid layout is attested.

## Pros and cons of the options

### Option 2: a module of `ltk_mapgeo`

- Good: the workspace gets no new crate. Both formats belong to a map.
- Bad: every `ltk_mapgeo` consumer compiles the grid format. The two formats share no type and
  no framing. The client loads them through different paths.

### Option 3: a read-only parser

- Good: the crate has no write path to validate.
- Bad: a consumer that edits the walkable area of a map has no way to write the result. A
  byte-for-byte round trip is the test that catches a wrong field offset.
