# Implementation Plan: ltk_inibin

**Branch**: `001-inibin-crate` | **Date**: 2026-03-26 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/001-inibin-crate/spec.md`

## Summary

Implement a new `ltk_inibin` crate for parsing, writing, and modifying inibin/troybin binary configuration files. Add SDBM hash algorithm to `ltk_hash::sdbm`. Re-export through the `league-toolkit` umbrella crate behind an `inibin` feature flag. This plan addresses PR #122 review feedback: rename bitfield to `ValueFlags`, add unified `as_*()` accessors on `InibinValue`, use `AsRef<str>` for SDBM functions, and expose `.keys()`/`.values()`/`.iter()` on collection types.

## Technical Context

**Language/Version**: Rust (workspace edition, same as other `ltk_*` crates)
**Primary Dependencies**: `thiserror` (errors), `byteorder` (binary I/O), `ltk_io_ext` (reader/writer extensions), `ltk_hash` (SDBM hashing), `glam` (Vec2/Vec3/Vec4 for vector set types), `bitflags` (ValueFlags), `indexmap` (ordered key-value storage)
**Storage**: N/A (in-memory data structures, binary file I/O)
**Testing**: `cargo test` — round-trip tests as primary verification
**Target Platform**: All Rust-supported platforms (no platform-specific code)
**Project Type**: Library (Rust crate within workspace)
**Performance Goals**: N/A — correctness and round-trip integrity are primary goals
**Constraints**: Must follow workspace conventions (`from_reader`/`to_writer`, `thiserror`, workspace deps)
**Scale/Scope**: Single crate (~10 source files), one `ltk_hash` module addition, one umbrella feature flag

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Crate-First Architecture | PASS | New `ltk_inibin` crate under `crates/`, independently compilable, depends on foundation crates only |
| II. Round-Trip Correctness | PASS | Round-trip tests planned (FR-012), `approx` not needed (integer/string formats), `insta` for snapshots if applicable |
| III. Strict CI Quality Gate | PASS | Will pass fmt, clippy -D warnings, and test before merge |
| IV. Idiomatic Rust I/O | PASS | `from_reader(&mut impl Read + Seek)` / `to_writer(&mut impl Write)`, builder not needed (simple struct construction) |
| V. Workspace Dependency Hygiene | PASS | All deps (`indexmap`, `bitflags`, etc.) added at workspace level first |
| Error Handling & Safety | PASS | Own error type via `thiserror`, `Result<T>` alias, no unwrap in lib code |
| Development Workflow | PASS | Feature branch `001-inibin-crate`, conventional commits, PR-based |

No violations. Gate passes.

## Project Structure

### Documentation (this feature)

```text
specs/001-inibin-crate/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── contracts/           # Phase 1 output (public API contracts)
└── tasks.md             # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

```text
crates/
├── ltk_hash/
│   └── src/
│       ├── sdbm.rs          # New: SDBM hash algorithm (AsRef<str>)
│       └── lib.rs            # Updated: re-export sdbm module
├── ltk_inibin/
│   ├── src/
│   │   ├── lib.rs            # Re-exports + module declarations
│   │   ├── error.rs          # InibinError enum (thiserror)
│   │   ├── inibin.rs         # Inibin top-level container (from_reader/to_writer)
│   │   ├── section.rs        # InibinSection — typed set with .keys()/.values()/.iter()
│   │   ├── value.rs          # InibinValue enum + unified as_*() accessors
│   │   └── value_flags.rs    # ValueFlags bitfield (bitflags)
│   ├── tests/
│   │   └── round_trip.rs     # Round-trip integration tests
│   └── Cargo.toml
└── league-toolkit/
    └── Cargo.toml            # Updated: add `inibin` feature flag
```

**Structure Decision**: Standard `ltk_*` crate layout following workspace conventions. SDBM hash lives in `ltk_hash` (centralized hashing). No builder pattern needed — direct struct construction suffices for inibin's flat key-value model.

## Complexity Tracking

No constitution violations to justify.
