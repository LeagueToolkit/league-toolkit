# ltk_inibin

Rust library for reading, writing, and modifying League of Legends **inibin** and **troybin** binary configuration files.

Part of the [League Toolkit](https://github.com/LeagueToolkit/league-toolkit) workspace.

## Overview

Inibin (`.inibin`) and troybin (`.troybin`) are legacy binary key-value formats used by the League of Legends engine for champion stats, spell data, item properties, particle effects, and map configuration. Both file extensions share the same binary format.

This crate provides:

- Parsing of version 1 (legacy, read-only) and version 2 (read+write) inibin files
- Key-based public API for reading, inserting, updating, and deleting values
- Round-trip integrity: parse a file and write it back to produce identical output
- Support for all 14 value set types

## Installation

```toml
[dependencies]
ltk_inibin = "0.1"
```

Or via the umbrella crate:

```toml
[dependencies]
league-toolkit = { version = "0.2", features = ["inibin"] }
```

## Quick Start

### Reading an inibin file

```rust,no_run
use std::fs::File;
use std::io::BufReader;
use ltk_inibin::Inibin;

let file = File::open("data/characters/annie/annie.inibin").unwrap();
let mut reader = BufReader::new(file);
let inibin = Inibin::from_reader(&mut reader).unwrap();

// Look up a value by its u32 hash key
if let Some(value) = inibin.get(0xABCD1234) {
    println!("Value: {:?}", value);
}
```

### Hashing section/property keys

Inibin keys are SDBM hashes of `section*property` (lowercased, `*` as delimiter). Use `ltk_hash::sdbm` to compute them:

```rust,ignore
use ltk_hash::sdbm;

let key = sdbm::hash_lower_with_delimiter("DATA", "AttackRange", '*');
let value = inibin.get(key);
```

### Modifying values

```rust
use ltk_inibin::{Inibin, Value};

let mut inibin = Inibin::new();

// Insert values of different types
inibin.insert(0x0001, 550.0f32);
inibin.insert(0x0002, 42i32);
inibin.insert(0x0003, "hello");
inibin.insert(0x0004, 9999999999i64);

// Remove a value
inibin.remove(0x0001);

// Update: re-inserting with a different type migrates across buckets
inibin.insert(0x0002, 3.125f32);
```

### Writing an inibin file

```rust,no_run
use std::fs::File;
use std::io::BufWriter;
use ltk_inibin::Inibin;

# let inibin = Inibin::new();
let file = File::create("output.inibin").unwrap();
let mut writer = BufWriter::new(file);
inibin.to_writer(&mut writer).unwrap();
```

### Round-trip

```rust
use std::io::Cursor;
use ltk_inibin::{Inibin, Value};

let mut file = Inibin::new();
file.insert(0x0001, 42i32);

let mut buf = Vec::new();
file.to_writer(&mut buf).unwrap();

let mut cursor = Cursor::new(&buf);
let file2 = Inibin::from_reader(&mut cursor).unwrap();

assert_eq!(file2.get(0x0001), Some(&Value::I32(42)));
```

### Iterating values

```rust
use ltk_inibin::{Inibin, Value, ValueFlags};

let mut inibin = Inibin::new();
inibin.insert(0x0001, 1i32);
inibin.insert(0x0002, 2.0f32);

// Iterate all key-value pairs across all buckets
for (key, value) in inibin.iter() {
    println!("0x{:08X} = {:?}", key, value);
}

// Access a specific section
if let Some(int_section) = inibin.section(ValueFlags::INT32_LIST) {
    println!("Int32 section has {} entries", int_section.len());
    // Use .keys(), .values(), or .iter()
    for key in int_section.keys() {
        println!("  key: 0x{:08X}", key);
    }
}
```

## Binary Format

### Version 2 (canonical, read+write)

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0 | 1 | `u8` | Version (`2`) |
| 1 | 2 | `u16` LE | String data length |
| 3 | 2 | `u16` LE | Flags bitfield (14 bits) |

Followed by set data for each flag bit that is set (in bit order 0-13), with StringList (bit 12) always last.

### Version 1 (legacy, read-only)

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0 | 1 | `u8` | Version (`1`) |
| 1 | 3 | `[u8; 3]` | Padding |
| 4 | 4 | `u32` LE | Value count |
| 8 | 4 | `u32` LE | String data length |

Followed by `value_count` hash keys (`u32` LE), then a single StringList set.

### Non-String Set Layout

Each non-string set is:
1. `u16` LE — value count
2. `count` x `u32` LE — hash keys
3. Value data (format depends on set type)

### StringList Set Layout

1. `u16` LE — value count
2. `count` x `u32` LE — hash keys
3. `count` x `u16` LE — string offsets (relative to string data start)
4. Null-terminated ASCII string data

## Value Set Types

All 14 types and their corresponding flag bits:

| Flag | Bit | Type | Rust Variant | Encoding |
|------|-----|------|-------------|----------|
| `INT32_LIST` | 0 | `i32` | `Value::I32` | 4 bytes LE |
| `F32_LIST` | 1 | `f32` | `Value::F32` | 4 bytes LE |
| `U8_LIST` | 2 | `u8` | `Value::U8` | 1 byte raw; `as_f32()` returns `byte * 0.1` (0.0-25.5) |
| `INT16_LIST` | 3 | `i16` | `Value::I16` | 2 bytes LE |
| `INT8_LIST` | 4 | `u8` | `Value::I8` | 1 byte |
| `BIT_LIST` | 5 | `bool` | `Value::Bool` | 8 booleans packed per byte |
| `VEC3_U8_LIST` | 6 | `[u8; 3]` | `Value::Vec3U8` | 3 bytes raw; `as_vec3()` decodes |
| `VEC3_F32_LIST` | 7 | `Vec3` | `Value::Vec3F32` | 3x `f32` LE |
| `VEC2_U8_LIST` | 8 | `[u8; 2]` | `Value::Vec2U8` | 2 bytes raw; `as_vec2()` decodes |
| `VEC2_F32_LIST` | 9 | `Vec2` | `Value::Vec2F32` | 2x `f32` LE |
| `VEC4_U8_LIST` | 10 | `[u8; 4]` | `Value::Vec4U8` | 4 bytes raw; `as_vec4()` decodes |
| `VEC4_F32_LIST` | 11 | `Vec4` | `Value::Vec4F32` | 4x `f32` LE |
| `STRING_LIST` | 12 | `String` | `Value::String` | Null-terminated ASCII with offset table |
| `INT64_LIST` | 13 | `i64` | `Value::I64` | 8 bytes LE |

### U8 (Fixed-Point Float) Encoding

The `U8` types (flags 2, 6, 8, 10) store floats as raw bytes. Use the unified `as_*()` accessors to decode:
- `Value::U8(byte)` → `as_f32()` returns `byte * 0.1` (range 0.0 to 25.5)
- `Value::Vec2U8([a, b])` → `as_vec2()` returns `Vec2::new(a * 0.1, b * 0.1)`
- `Value::Vec3U8([a, b, c])` → `as_vec3()` returns `Vec3::new(a * 0.1, b * 0.1, c * 0.1)`
- `Value::Vec4U8([a, b, c, d])` → `as_vec4()` returns `Vec4::new(a * 0.1, b * 0.1, c * 0.1, d * 0.1)`

These accessors also work on the non-packed variants (`F32`, `Vec2F32`, etc.), returning the value directly.

### BitList Encoding

Booleans are packed 8 per byte. For a set with `n` values, `ceil(n / 8)` bytes are read/written. Bits are extracted in order from LSB to MSB within each byte.

## Architecture

### Bucket-Based Storage

Internally, `Inibin` stores data in **sections** — one `Section` per active flag type. Each section holds an `IndexMap<u32, Value>` where keys are SDBM hashes.

The public API is **key-based**: methods like `get`, `insert`, and `remove` search across all buckets transparently. When inserting a value, the library routes it to the correct bucket based on the value's type. If a key already exists in a different-type bucket, it is removed from the old bucket first.

### Error Handling

```rust
pub enum Error {
    UnsupportedVersion(u8),  // Version byte is not 1 or 2
    U8FloatOverflow(f32),    // Fixed-point float outside 0.0-25.5 on write
    Io(std::io::Error),      // Underlying I/O error
}
```

### Trait Bounds

- **Reading**: `from_reader<R: Read + Seek>` — requires seeking for StringList offset resolution
- **Writing**: `to_writer<W: Write>` — sequential writes only

## Hash Algorithm

Inibin keys use **SDBM** hashing (multiplier 65599) applied to lowercased `section*property` strings:

```
hash = 0
for each byte in lowercase("section*property"):
    hash = byte + (hash << 6) + (hash << 16) - hash
```

The hash implementation lives in `ltk_hash::sdbm`. Use `hash_lower_with_delimiter(section, property, '*')` to compute keys.

## Name Resolution

For resolving hash keys back to human-readable `(section, property)` pairs, see the companion crate [`ltk_inibin_names`](../ltk_inibin_names/).

## License

MIT OR Apache-2.0
