# Feature Specification: Inibin File Parser (ltk_inibin)

**Feature Branch**: `001-inibin-crate`
**Created**: 2026-03-25
**Status**: Draft
**Input**: User description: "We need to implement a new ltk crate for reading inibin files - https://github.com/LeagueToolkit/league-toolkit/issues/119"

## Clarifications

### Session 2026-03-25

- Q: What should the public API style be for value access? → A: Key-based public API for read/edit/delete; internal representation stays bucket-based for efficiency.
- Q: Should the parser require Read + Seek or just Read? → A: Read + Seek, consistent with workspace conventions for offset-based formats.
- Q: Where should the SDBM hash implementation live? → A: In `ltk_hash`, centralized alongside FNV-1a and ELF.
- Q: Where should the inibin fixlist (hash→name mapping) live? → A: Separate `ltk_inibin_names` crate, keeping `ltk_inibin` lean for users who only need parsing.
- Q: How should the fixlist data be stored in Rust? → A: Compile-time static map using `phf` (perfect hash function) for zero-cost lookups.
- Q: Should Int64 (flag 13) support reading and writing, or just reading? → A: Full read+write support, maintaining round-trip integrity.

### Session 2026-03-26

- Q: Should `ltk_inibin_names` be included in this PR? → A: No. Descoped to a separate PR — too large for the current scope. This PR covers `ltk_inibin` only (parsing, writing, modification, Int64 support).
- Q: What map type should be used for internal storage? → A: `IndexMap` — preserves insertion order for deterministic iteration and serialization.
- Q: How should packed floats (U8 types) be stored internally? → A: Store raw `u8` byte, provide `as_f32()` accessor returning `byte * 0.1`. Lossless round-trip, validation implicit.
- Q: Should the API use generics for ergonomics? → A: Yes — `From<T>` impls on `InibinValue` for construction + typed getter methods (`get_i32()`, `get_f32()`, etc.) on `Inibin` for extraction.

### Session 2026-03-26 (PR #122 review)

- Q: What should the value-type bitfield be named? → A: `ValueFlags` — concise and accurately conveys bitfield semantics.
- Q: Should `InibinValue` provide unified `as_*()` accessors that handle both packed and non-packed variants? → A: Yes — `as_f32()` returns `f32` from both `Float32` and `U8` (packed) variants; same pattern for vec types.
- Q: Should SDBM hash functions accept `AsRef<str>` instead of `&str`? → A: Yes — `AsRef<str>` for SDBM functions only; other `ltk_hash` functions unchanged for now.
- Q: Should section/set types expose `.keys()` and `.values()` iterator methods? → A: Yes — expose `.keys()`, `.values()`, and `.iter()` on all collection types.

### Session 2026-03-26 (DX)

- Q: Should there be a convenience function that defaults the `*` delimiter for SDBM inibin key hashing? → A: Yes — `ltk_hash::sdbm::hash_inibin_key(section, property)` centralized in the hash crate, defaults `*` as delimiter.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Read Inibin Files (Priority: P1)

As a developer working with League of Legends legacy files, I want to parse inibin/troybin files into a structured in-memory representation so I can inspect and extract configuration values by their hashed keys.

**Why this priority**: Reading is the fundamental capability. Without parsing, no other operations are possible. This unlocks inspection, migration, and tooling workflows for legacy game data.

**Independent Test**: Can be fully tested by providing binary inibin files and verifying that all value sets and their keyed entries are correctly parsed into the expected types and values.

**Acceptance Scenarios**:

1. **Given** a valid version 2 inibin file with multiple value sets, **When** the file is parsed, **Then** all sets are correctly identified by their flags and all key-value pairs within each set contain the expected typed values.
2. **Given** a valid version 1 (legacy) inibin file, **When** the file is parsed, **Then** the string list set is correctly read with all key-value pairs intact.
3. **Given** a troybin file (identical format to inibin), **When** the file is parsed, **Then** it produces the same structured output as an equivalent inibin file.

---

### User Story 2 - Access Values by Hash Key (Priority: P1)

As a developer, I want to look up values from parsed inibin data using hash keys (section/property pairs hashed with SDBM) so I can retrieve specific configuration entries without iterating all sets manually.

**Why this priority**: Direct key-based access is the primary use case for consumers of inibin data. Without it, the parsed structure has limited utility.

**Independent Test**: Can be tested by parsing an inibin file and querying specific known hash keys, verifying the returned values match expected data.

**Acceptance Scenarios**:

1. **Given** a parsed inibin file, **When** a value is requested by its hash key, **Then** the correct typed value is returned from the appropriate set.
2. **Given** a parsed inibin file, **When** a value is requested by a non-existent hash key, **Then** a None/empty result is returned without error.

---

### User Story 3 - Write Inibin Files (Priority: P2)

As a developer, I want to write inibin data back to binary format so I can create or modify legacy configuration files for modding or tooling purposes.

**Why this priority**: Writing enables round-trip workflows (read-modify-write) and creation of new inibin files. This is important but secondary to reading.

**Independent Test**: Can be tested by constructing an inibin structure in memory, writing it to binary, then re-reading it and verifying all values are preserved (round-trip test).

**Acceptance Scenarios**:

1. **Given** an in-memory inibin structure with multiple value sets, **When** written to binary format, **Then** the output is a valid version 2 inibin file that can be re-parsed to produce identical data.
2. **Given** a parsed inibin file, **When** written back to binary and re-parsed, **Then** all values match the original (round-trip integrity).

---

### User Story 4 - Modify Inibin Data (Priority: P2)

As a developer, I want to insert, remove, and update values in an inibin structure so I can programmatically edit legacy configuration data.

**Why this priority**: Modification support enables modding workflows and tooling that transforms inibin data.

**Independent Test**: Can be tested by constructing an inibin structure, performing insertions/removals/updates, then verifying the structure reflects the changes correctly.

**Acceptance Scenarios**:

1. **Given** a parsed inibin structure, **When** a new value is added with a specific hash key and type, **Then** it is placed in the correct value set bucket and can be retrieved by key.
2. **Given** a parsed inibin structure with existing entries, **When** an entry is removed by hash key, **Then** it is no longer present in the structure.

---

### Edge Cases

- What happens when an inibin file has a version byte other than 1 or 2? The parser should return an error indicating an unsupported version.
- What happens when a BitList set has a value count that is not a multiple of 8? The parser should correctly handle partial bytes by reading only the relevant bits.
- What happens when a FixedPointFloat value would overflow its byte range (0-255) during writing? The writer should return an error.
- What happens when string data contains non-ASCII characters? The parser should handle them gracefully or return an error, since the format uses null-terminated ASCII strings.
- What happens when an inibin file is truncated or corrupted? The parser should return a descriptive error rather than panicking.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The library MUST parse version 2 inibin files from a seekable reader (`Read + Seek`), reading the header (version byte, string data length, flags) and all value sets indicated by the flags bitfield.
- **FR-002**: The library MUST parse version 1 (legacy) inibin files, reading the header (version byte, padding, value count, string data length) and the single string list set.
- **FR-003**: The library MUST support all 14 value set types: Int32List, Float32List, U8List, Int16List, Int8List, BitList, Vec3U8List, Vec3F32List, Vec2U8List, Vec2F32List, Vec4U8List, Vec4F32List, StringList, and Int64List (flag 13).
- **FR-004**: The library MUST store parsed data in a bucket-based representation where each value set type maps to a collection of hash-key/value pairs.
- **FR-005**: The library MUST provide a key-based public API that allows users to read values by hash key, searching across all internal buckets transparently.
- **FR-006**: The library MUST provide a key-based public API that allows users to insert, update, and delete values by hash key, with the library routing to the correct internal bucket based on value type.
- **FR-007**: The library MUST write inibin data to binary format as version 2, computing the correct flags bitfield and string data length.
- **FR-008**: The library MUST correctly handle BitList encoding and decoding (8 boolean values packed per byte).
- **FR-009**: The library MUST correctly handle FixedPointFloat encoding and decoding (byte value multiplied by 0.1, range 0.0-25.5).
- **FR-010**: The library MUST correctly handle StringList encoding and decoding (null-terminated ASCII strings with offset-based addressing).
- **FR-011**: The library MUST return descriptive errors for unsupported versions, corrupted data, and invalid operations (e.g., FixedPointFloat overflow).
- **FR-012**: The library MUST support round-trip integrity: parsing a file and writing it back should produce binary-identical output (for version 2 files).
- **FR-013**: The library MUST support Int64 (flag 13, `i64`) values for both reading and writing, following the same pattern as other numeric set types.
- **FR-014**: The SDBM hash functions in `ltk_hash::sdbm` MUST accept `AsRef<str>` for ergonomic use with `String`, `&str`, `Cow<str>`, etc.
- **FR-014a**: `ltk_hash::sdbm` MUST provide a `hash_inibin_key(section, property)` convenience function that defaults the `*` delimiter, equivalent to `hash_lower_with_delimiter(section, property, '*')`.
- **FR-015**: `InibinValue` MUST provide unified `as_f32()`, `as_vec2()`, `as_vec3()`, `as_vec4()` accessors that transparently convert from both packed (U8-based) and non-packed variants.
- **FR-016**: All collection types (sets/sections) MUST expose `.keys()`, `.values()`, and `.iter()` iterator methods following idiomatic Rust map conventions.
- ~~**FR-017**: `ltk_inibin_names` crate~~ — **Descoped** to a separate PR.
- ~~**FR-018**: `ltk_inibin_names` lookup function~~ — **Descoped** to a separate PR.

### Key Entities

- **InibinFile**: The top-level container representing a parsed inibin/troybin file. Holds a collection of value sets keyed by their type flag.
- **InibinSet**: A typed collection of key-value pairs where keys are u32 hashes and values are of the type indicated by the set's flag (e.g., i32, f32, string, vector types). Exposes `.keys()`, `.values()`, and `.iter()` iterator methods.
- **ValueFlags** (formerly `InibinFlags`): A bitfield representing the 14 possible value set types present in an inibin file (flags 0-13).
- **InibinValue**: The typed value stored in a set entry (integer, float, u8 fixed-point float, boolean, string, i64, or vector variant). Provides unified `as_*()` accessors (e.g., `as_f32()`, `as_vec2()`) that transparently handle both packed (U8-based) and non-packed variants.
- ~~**InibinNames** (in `ltk_inibin_names`)~~ — **Descoped** to a separate PR.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All 14 value set types (including Int64) can be correctly parsed and written without data loss.
- **SC-002**: Round-trip parsing (read-write-read) produces identical data for all supported value set types.
- **SC-003**: Both version 1 and version 2 inibin files are parseable.
- **SC-004**: Value lookup by hash key returns correct results for all set types.
- **SC-005**: Invalid or corrupted input produces clear error messages rather than panics.
- **SC-006**: The library integrates into the league-toolkit workspace and passes all CI checks (formatting, linting, tests).
- **SC-007**: Unified `as_*()` accessors on `InibinValue` return correct values for both packed and non-packed storage variants.
- ~~**SC-008**: `ltk_inibin_names` hash-to-name resolution~~ — **Descoped** to a separate PR.

## Assumptions

- The inibin and troybin formats are binary-identical and can be handled by the same parser without format-specific logic.
- Hash keys use the SDBM hash algorithm (added to `ltk_hash`) applied to lowercased section/property pairs joined by '*' delimiter, consistent with the reference implementation.
- Version 2 is the canonical write format; version 1 is read-only (legacy support).
- The library follows existing workspace conventions: `from_reader`/`to_writer` pattern, `thiserror` error types, and workspace-level dependency management.
- String data in inibin files uses ASCII encoding with null terminators.
- Hash-to-name resolution (`ltk_inibin_names`) is deferred to a separate PR.
