# Implementation Plan: Inibin File Parser (ltk_inibin + ltk_inibin_names)

**Branch**: `001-inibin-crate` | **Date**: 2026-03-25 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/001-inibin-crate/spec.md`

## Summary

Implement `ltk_inibin` — a Rust crate for reading, writing, and modifying League of Legends inibin/troybin binary files. Supports 14 value set types (including Int64 at flag 13), version 1 (read-only) and version 2 (read+write) formats, with key-based public API and bucket-based internal storage. Additionally, implement `ltk_inibin_names` — a companion crate providing compile-time hash→name resolution from the lolpytools fixlist.

## Technical Context

**Language/Version**: Rust (workspace edition, same as other `ltk_*` crates)
**Primary Dependencies**: `thiserror` (errors), `byteorder` (binary I/O), `ltk_io_ext` (reader/writer extensions), `ltk_hash` (SDBM hashing), `glam` (Vec2/Vec3/Vec4 for vector set types), `bitflags` (InibinFlags), `phf`/`phf_codegen` (compile-time hash map for ltk_inibin_names)
**Storage**: N/A (in-memory data structures, binary file I/O)
**Testing**: `cargo test`, `approx` for floating-point comparisons
**Target Platform**: All platforms supported by the workspace (no platform-specific code)
**Project Type**: Library (two crates)
**Performance Goals**: Zero-cost name lookups via `phf`; standard binary I/O performance
**Constraints**: Must follow workspace conventions (from_reader/to_writer, thiserror, glam, workspace deps)
**Scale/Scope**: ~14 value types, ~thousands of fixlist entries

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Crate-First Architecture | PASS | Two separate crates: `ltk_inibin` (parser) and `ltk_inibin_names` (name resolution). Both under `crates/`. No circular deps. Umbrella re-exports via feature flags. |
| II. Round-Trip Correctness | PASS | All 14 types support read+write. Round-trip tests required. `approx` for floats. |
| III. Strict CI Quality Gate | PASS | fmt + clippy + test required before merge. |
| IV. Idiomatic Rust I/O | PASS | `from_reader(Read+Seek)` / `to_writer(Write)`. `glam` vectors. |
| V. Workspace Dependency Hygiene | PASS | All existing deps at workspace level. `phf`/`phf_codegen` added at workspace level (justified: thousands of static entries, zero-cost lookups). |
| Error Handling & Safety | PASS | Own error type via thiserror. No unwrap in lib code. |

**Post-design re-check**: All gates still PASS.

## Project Structure

### Documentation (this feature)

```text
specs/001-inibin-crate/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   └── public-api.md
└── tasks.md             # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

```text
crates/ltk_hash/
├── src/
│   ├── lib.rs           # Module declarations (add sdbm)
│   └── sdbm.rs          # SDBM hash implementation
└── Cargo.toml

crates/ltk_inibin/
├── src/
│   ├── lib.rs           # Re-exports + module declarations
│   ├── error.rs         # InibinError + Result
│   ├── file.rs          # InibinFile (from_reader, to_writer, CRUD API)
│   ├── flags.rs         # InibinFlags bitfield
│   ├── set.rs           # InibinSet (per-bucket read/write logic)
│   └── value.rs         # InibinValue enum
├── tests/
│   └── round_trip.rs    # Integration round-trip tests
└── Cargo.toml

crates/ltk_inibin_names/
├── src/
│   └── lib.rs           # lookup() function + include generated phf map
├── build.rs             # phf_codegen: generate hash→name map at compile time
├── data/
│   └── fixlist.rs       # Raw fixlist data (section, name, hash) tuples
└── Cargo.toml

crates/league-toolkit/
├── Cargo.toml           # Add inibin + inibin-names feature flags
└── src/lib.rs           # Re-export ltk_inibin and ltk_inibin_names
```

**Structure Decision**: Two new crates under `crates/` following the existing workspace pattern. `ltk_inibin` is the core parser/writer with no dependency on names. `ltk_inibin_names` is a standalone lookup crate using `phf` for compile-time hash maps. Both are re-exported through the umbrella crate behind feature flags.

## Complexity Tracking

No constitution violations — table not needed.
