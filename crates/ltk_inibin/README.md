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
use ltk_inibin::InibinFile;

let file = File::open("data/characters/annie/annie.inibin").unwrap();
let mut reader = BufReader::new(file);
let inibin = InibinFile::from_reader(&mut reader).unwrap();

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
use ltk_inibin::{InibinFile, InibinValue};

let mut inibin = InibinFile::new();

// Insert values of different types
inibin.insert(0x0001, InibinValue::F32(550.0));
inibin.insert(0x0002, InibinValue::Int32(42));
inibin.insert(0x0003, InibinValue::String("hello".to_string()));
inibin.insert(0x0004, InibinValue::Int64(9999999999));

// Remove a value
inibin.remove(0x0001);

// Update: re-inserting with a different type migrates across buckets
inibin.insert(0x0002, InibinValue::F32(3.14));
```

### Writing an inibin file

```rust,no_run
use std::fs::File;
use std::io::BufWriter;
use ltk_inibin::InibinFile;

# let inibin = InibinFile::new();
let file = File::create("output.inibin").unwrap();
let mut writer = BufWriter::new(file);
inibin.to_writer(&mut writer).unwrap();
```

### Round-trip

```rust
use std::io::Cursor;
use ltk_inibin::{InibinFile, InibinValue};

let mut file = InibinFile::new();
file.insert(0x0001, InibinValue::Int32(42));

let mut buf = Vec::new();
file.to_writer(&mut buf).unwrap();

let mut cursor = Cursor::new(&buf);
let file2 = InibinFile::from_reader(&mut cursor).unwrap();

assert_eq!(file2.get(0x0001), Some(&InibinValue::Int32(42)));
```

### Iterating values

```rust
use ltk_inibin::{InibinFile, InibinValue, InibinFlags};

let mut inibin = InibinFile::new();
inibin.insert(0x0001, InibinValue::Int32(1));
inibin.insert(0x0002, InibinValue::F32(2.0));

// Iterate all key-value pairs across all buckets
for (key, value) in inibin.iter() {
    println!("0x{:08X} = {:?}", key, value);
}

// Access a specific set bucket
if let Some(int_set) = inibin.set(InibinFlags::INT32_LIST) {
    println!("Int32 set has {} entries", int_set.len());
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
| `INT32_LIST` | 0 | `i32` | `InibinValue::Int32` | 4 bytes LE |
| `F32_LIST` | 1 | `f32` | `InibinValue::F32` | 4 bytes LE |
| `U8_LIST` | 2 | `f32` | `InibinValue::U8` | 1 byte, `value * 0.1` (range 0.0-25.5) |
| `INT16_LIST` | 3 | `i16` | `InibinValue::Int16` | 2 bytes LE |
| `INT8_LIST` | 4 | `u8` | `InibinValue::Int8` | 1 byte |
| `BIT_LIST` | 5 | `bool` | `InibinValue::Bool` | 8 booleans packed per byte |
| `VEC3_U8_LIST` | 6 | `Vec3` | `InibinValue::Vec3U8` | 3 bytes, each `* 0.1` |
| `VEC3_F32_LIST` | 7 | `Vec3` | `InibinValue::Vec3F32` | 3x `f32` LE |
| `VEC2_U8_LIST` | 8 | `Vec2` | `InibinValue::Vec2U8` | 2 bytes, each `* 0.1` |
| `VEC2_F32_LIST` | 9 | `Vec2` | `InibinValue::Vec2F32` | 2x `f32` LE |
| `VEC4_U8_LIST` | 10 | `Vec4` | `InibinValue::Vec4U8` | 4 bytes, each `* 0.1` |
| `VEC4_F32_LIST` | 11 | `Vec4` | `InibinValue::Vec4F32` | 4x `f32` LE |
| `STRING_LIST` | 12 | `String` | `InibinValue::String` | Null-terminated ASCII with offset table |
| `INT64_LIST` | 13 | `i64` | `InibinValue::Int64` | 8 bytes LE |

### U8 (Fixed-Point Float) Encoding

The `U8` types (flags 2, 6, 8, 10) store floats as single bytes scaled by 0.1:
- **Read**: `byte as f32 * 0.1` (range 0.0 to 25.5)
- **Write**: `(value / 0.1).round() as u8` (validated to 0.0-25.5, returns `InibinError::U8FloatOverflow` if out of range)

### BitList Encoding

Booleans are packed 8 per byte. For a set with `n` values, `ceil(n / 8)` bytes are read/written. Bits are extracted in order from LSB to MSB within each byte.

## Architecture

### Bucket-Based Storage

Internally, `InibinFile` stores data in **buckets** — one `InibinSet` per active flag type. Each set holds a `HashMap<u32, InibinValue>` where keys are SDBM hashes.

The public API is **key-based**: methods like `get`, `insert`, and `remove` search across all buckets transparently. When inserting a value, the library routes it to the correct bucket based on the value's type. If a key already exists in a different-type bucket, it is removed from the old bucket first.

### Error Handling

```rust
pub enum InibinError {
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
