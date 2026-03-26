# Data Model: ltk_inibin

**Phase**: 1 | **Date**: 2026-03-25 | **Updated**: 2026-03-26

## Entities

### InibinFile

The top-level container for an inibin/troybin file.

**Fields**:
- `sections`: Map from `ValueFlags` (single flag value) to `Section` — the bucket-based internal storage

**Relationships**: Contains zero or more `Section` instances, keyed by flag type. At most 14 sections (one per flag bit).

**Lifecycle**:
- Created via `from_reader` (parsing) or direct construction
- Modified via key-based insert/update/delete API
- Serialized via `to_writer` (always writes as version 2)

**Identity**: An InibinFile is a value type — no unique identity beyond its contents.

### Section (formerly InibinSet)

A typed collection of key-value pairs within a single bucket.

**Fields**:
- `kind`: `ValueFlags` — identifies which value type this section holds
- `properties`: `IndexMap<u32, Value>` — preserves insertion order

**Relationships**: Owned by `Inibin`. Each section holds values of exactly one type.

**Public iterators**: `.keys()`, `.values()`, `.iter()` — idiomatic map-like access.

**Validation**:
- Hash keys must be unique within a section
- Values must match the section's type constraint
- U8 (fixed-point float) values must be in range 0-255 (raw byte storage)

### ValueFlags (formerly InibinFlags)

Bitfield (u16) representing value set types.

**Values** (14 bits):
- Bit 0: `INT32_LIST`
- Bit 1: `F32_LIST`
- Bit 2: `U8_LIST`
- Bit 3: `INT16_LIST`
- Bit 4: `INT8_LIST`
- Bit 5: `BIT_LIST`
- Bit 6: `VEC3_U8_LIST`
- Bit 7: `VEC3_F32_LIST`
- Bit 8: `VEC2_U8_LIST`
- Bit 9: `VEC2_F32_LIST`
- Bit 10: `VEC4_U8_LIST`
- Bit 11: `VEC4_F32_LIST`
- Bit 12: `STRING_LIST`
- Bit 13: `INT64_LIST`

**Usage**: In the file header (v2), a combined flags value indicates which sets are present. Internally, each set is keyed by a single flag value.

### Value (formerly InibinValue)

Typed value enum representing all possible value types.

**Variants**:
- `I32(i32)`
- `F32(f32)`
- `U8(u8)` — raw byte storage; `as_f32()` returns `byte * 0.1`
- `I16(i16)`
- `I8(u8)`
- `Bool(bool)`
- `Vec3U8([u8; 3])` — raw bytes; `as_vec3()` returns packed floats
- `Vec3F32(Vec3)`
- `Vec2U8([u8; 2])` — raw bytes; `as_vec2()` returns packed floats
- `Vec2F32(Vec2)`
- `Vec4U8([u8; 4])` — raw bytes; `as_vec4()` returns packed floats
- `Vec4F32(Vec4)`
- `String(String)`
- `I64(i64)`

**Unified accessors**: `as_f32()`, `as_vec2()`, `as_vec3()`, `as_vec4()` handle both packed (U8-based) and non-packed variants transparently.

**Mapping**: Each variant corresponds to exactly one `ValueFlags` value. The public API uses this enum for type-safe value access. The library determines which bucket to route to based on the variant.

### InibinNames (in `ltk_inibin_names`)

A compile-time static lookup table for resolving hash keys to human-readable names.

**Fields**:
- `INIBIN_NAMES`: `phf::Map<u32, (&'static str, &'static str)>` — maps hash → (section, name)

**Data Source**: Extracted from lolpytools `inibin_fix.py` `all_inibin_fixlist`.

**Lifecycle**: Static, immutable, baked into the binary at compile time.

## Binary Layout

### Version 2 File Header

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0 | 1 | u8 | Version (== 2) |
| 1 | 2 | u16 LE | String data length |
| 3 | 2 | u16 LE | Flags bitfield |

Followed by set data for each flag bit set (in order), with StringList last.

### Version 1 File Header (read-only)

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0 | 1 | u8 | Version (== 1) |
| 1 | 3 | [u8; 3] | Padding |
| 4 | 4 | u32 LE | Value count |
| 8 | 4 | u32 LE | String data length |

Followed by `value_count` x u32 hash keys, then a single StringList set.

### Non-String Set Layout

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0 | 2 | u16 LE | Value count |
| 2 | count*4 | [u32 LE] | Hash keys |
| ... | varies | varies | Value data (type-dependent) |

### StringList Set Layout

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0 | 2 | u16 LE | Value count |
| 2 | count*4 | [u32 LE] | Hash keys |
| ... | count*2 | [u16 LE] | String offsets (relative to string data start) |
| ... | varies | bytes | Null-terminated ASCII string data |
