# Research: ltk_inibin

**Phase**: 0 | **Date**: 2026-03-25 | **Updated**: 2026-03-26

## R-001: Inibin Binary Format

**Decision**: Follow the C# reference implementation exactly for binary layout.

**Rationale**: The reference implementation (LeagueToolkit C#) is the authoritative source for the inibin format. The format has two versions with well-defined headers and 14 value set types (13 original + Int64 at flag 13).

**Format Summary**:

### Version 2 (canonical)
- `u8` version (== 2)
- `u16` string data length
- `u16` flags bitfield (14 bits, one per set type)
- For each set bit in flags (in order, bits 0-12): read the corresponding InibinSet
- Int64List (bit 13) follows the same non-string set pattern as Int32List
- StringList (bit 12) is read last, using string data length to compute the string data offset

### Version 1 (legacy, read-only)
- `u8` version (== 1)
- `[u8; 3]` padding
- `u32` value count
- `u32` string data length
- Hashes are read externally (value_count x u32)
- Single StringList set

### InibinSet (non-string) read order:
1. `u16` value count
2. `value_count` x `u32` hash keys
3. Value data (format depends on set type)

### InibinSet (StringList) read order:
1. `u16` value count
2. `value_count` x `u32` hash keys
3. `value_count` x `u16` string offsets
4. String data (null-terminated ASCII at computed absolute offsets)

**Alternatives considered**: None — format is fixed by the game engine.

## R-002: SDBM Hash Algorithm

**Decision**: Implement SDBM hash in `ltk_hash::sdbm` module following the existing fnv1a/elf pattern.

**Rationale**: Inibin keys are SDBM hashes of `section*property` (lowercased, `*` as delimiter). Centralizing in `ltk_hash` keeps all hash algorithms together and makes the function reusable.

**Algorithm**:
```
hash = 0
for each byte in input.to_lowercase():
    hash = byte + (hash << 6) + (hash << 16) - hash
return hash as u32
```

The reference C# uses `Sdbm.HashLowerWithDelimiter(section, property, '*')` which concatenates `section + '*' + property`, lowercases, and hashes.

**Alternatives considered**: Keeping SDBM internal to ltk_inibin — rejected per clarification to centralize in ltk_hash.

## R-003: Value Type Encoding

**Decision**: Use an enum `InibinValue` with 14 variants matching the flag types.

**Rationale**: Maps directly to the format's 14 set types. Vector types use `glam` (Vec2/Vec3/Vec4) per constitution. Fixed-point floats (U8) are stored as `f32` after conversion (byte * 0.1) for ergonomic access, but validated on write (must be 0.0-25.5).

**Type mapping**:

| Flag | Rust Value Type | Read | Write |
|------|----------------|------|-------|
| INT32_LIST | `i32` | read_i32::<LE> | write_i32::<LE> |
| F32_LIST | `f32` | read_f32::<LE> | write_f32::<LE> |
| U8_LIST | `f32` | u8 * 0.1 | validate range, f32 / 0.1 as u8 |
| INT16_LIST | `i16` | read_i16::<LE> | write_i16::<LE> |
| INT8_LIST | `u8` | read_u8 | write_u8 |
| BIT_LIST | `bool` | bit extraction (8 per byte) | bit packing (8 per byte) |
| VEC3_U8_LIST | `Vec3` | 3x u8 * 0.1 | validate, 3x f32/0.1 as u8 |
| VEC3_F32_LIST | `Vec3` | 3x read_f32 | 3x write_f32 |
| VEC2_U8_LIST | `Vec2` | 2x u8 * 0.1 | validate, 2x f32/0.1 as u8 |
| VEC2_F32_LIST | `Vec2` | 2x read_f32 | 2x write_f32 |
| VEC4_U8_LIST | `Vec4` | 4x u8 * 0.1 | validate, 4x f32/0.1 as u8 |
| VEC4_F32_LIST | `Vec4` | 4x read_f32 | 4x write_f32 |
| STRING_LIST | `String` | null-terminated ASCII | null-terminated + offset table |
| INT64_LIST | `i64` | read_i64::<LE> | write_i64::<LE> |

**Alternatives considered**: Storing fixed-point as raw bytes — accepted in implementation (stores raw `u8`), but with unified `as_*()` accessors that handle conversion transparently.

**Update (2026-03-26, PR #122 review)**: The actual implementation stores U8 variants as raw `u8` bytes (not `f32`). Unified `as_f32()`, `as_vec2()`, `as_vec3()`, `as_vec4()` accessors handle conversion from both packed (U8) and non-packed (F32) variants transparently. This provides lossless round-trip while still offering ergonomic float access.

## R-009: ValueKind → ValueFlags Rename (PR #122 review)

**Decision**: Rename `ValueKind` to `ValueFlags` throughout `ltk_inibin`.
**Rationale**: The type is a `bitflags!` bitfield, not an enum of kinds. `ValueFlags` accurately describes its nature (reviewer feedback from `alanpq`).
**Impact**: Rename in `value_kind.rs` → `value_flags.rs`, update all references in `section.rs`, `file.rs`, `lib.rs`.

## R-010: SDBM Hash Functions Accept `AsRef<str>` (PR #122 review)

**Decision**: Change SDBM hash function signatures from `&str` to `impl AsRef<str>`.
**Rationale**: Ergonomic — allows passing `String`, `&str`, `Cow<str>` without `.as_str()` calls. Only applied to SDBM functions; other `ltk_hash` functions unchanged to limit scope.
**Alternatives considered**: Update all `ltk_hash` functions — deferred to separate PR.

## R-011: Unified `as_*()` Accessors on Value (PR #122 review)

**Decision**: Replace separate `u8_as_f32()`, `vec2_u8_as_f32()`, etc. with unified `as_f32()`, `as_vec2()`, `as_vec3()`, `as_vec4()`.
**Rationale**: Consumers shouldn't need to know the storage representation. `as_f32()` handles both `F32(v)` → `Some(v)` and `U8(b)` → `Some(b as f32 * 0.1)`.
**Alternatives considered**: Keep separate per-storage accessors — rejected; adds unnecessary cognitive load.

## R-012: `.keys()` / `.values()` on Section (PR #122 review)

**Decision**: Add `.keys()` and `.values()` methods to `Section` (`.iter()` already exists).
**Rationale**: Idiomatic Rust for map-like containers. Matches `HashMap`/`IndexMap` conventions. Delegates to `IndexMap::keys()` and `IndexMap::values()`.
**Alternatives considered**: `.iter()` only — rejected per reviewer feedback.

## R-004: Endianness

**Decision**: Little-endian for all multi-byte reads/writes.

**Rationale**: The C# reference uses `BinaryReader` which defaults to little-endian. League of Legends targets x86/x64 which is little-endian.

**Alternatives considered**: None — format dictates endianness.

## R-005: Existing Workspace Dependencies

**Decision**: Use workspace-level dependencies where available. Add `phf` at workspace level for the new `ltk_inibin_names` crate.

**Rationale**: `thiserror`, `byteorder`, `glam`, `bitflags` already exist at workspace level. `ltk_io_ext` and `ltk_hash` are path dependencies. The `phf` crate (with `phf_codegen` as build dependency) is needed for compile-time perfect hash maps in `ltk_inibin_names` — justified by the thousands of entries in the fixlist where runtime HashMap initialization would be wasteful.

**Alternatives considered**: `LazyLock` + `HashMap` — rejected per clarification; `phf` gives zero-cost lookups with no runtime initialization.

## R-006: Reader Trait Bounds

**Decision**: `from_reader` requires `Read + Seek` per clarification session.

**Rationale**: StringList reading uses offset-based seeking in the reference implementation. This is consistent with `ltk_wad` which also requires `Read + Seek` for offset-based formats. The `to_writer` method only needs `Write` since it writes sequentially.

**Alternatives considered**: `Read`-only with buffering — rejected per clarification.

## R-007: Int64 Support (Flag 13)

**Decision**: Add Int64List as flag bit 13, full read+write support.

**Rationale**: The lolpytools reference (`inibin2.py`) documents flag 13 as 64-bit long long (`int64`). While less common than other types, some inibin files in the wild use this type. Full read+write support maintains round-trip integrity, which is a core constitution principle.

**Format**: Identical to Int32List but with 8-byte values instead of 4. Each entry is `i64` read/written as little-endian.

**Alternatives considered**: Read-only — rejected per clarification; round-trip integrity requires write support.

## R-008: Inibin Name Resolution (ltk_inibin_names)

**Decision**: Separate `ltk_inibin_names` crate with compile-time `phf::Map` for hash→name lookups.

**Rationale**: The lolpytools `inibin_fix.py` contains thousands of known `(section, name)` mappings. A separate crate keeps `ltk_inibin` lean (no binary size overhead for users who don't need name resolution). The `phf` crate generates a perfect hash map at compile time via `phf_codegen` in `build.rs`, providing O(1) lookups with zero runtime initialization cost.

**Architecture**:
- `build.rs`: Uses `phf_codegen` to generate a `phf::Map<u32, (&str, &str)>` from the fixlist data
- `src/lib.rs`: Exposes `lookup(hash: u32) -> Option<(&str, &str)>` using the generated map
- Data source: Extracted from lolpytools `inibin_fix.py` `all_inibin_fixlist`

**Alternatives considered**:
- Inside `ltk_inibin` behind feature flag — rejected per clarification; separate crate preferred
- Runtime HashMap — rejected per clarification; compile-time phf preferred
- External data file — rejected; adds distribution complexity
