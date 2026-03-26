# Quickstart: ltk_inibin

**Phase**: 1 | **Date**: 2026-03-25 | **Updated**: 2026-03-26

## Reading an inibin file

```rust
use std::fs::File;
use std::io::BufReader;
use ltk_inibin::Inibin;

let file = File::open("data/characters/annie/annie.inibin")?;
let mut reader = BufReader::new(file);
let inibin = Inibin::from_reader(&mut reader)?;

// Look up a value by hash key
if let Some(value) = inibin.get(0xABCD1234) {
    println!("Value: {:?}", value);
}
```

## Hashing a section/property key

```rust
use ltk_hash::sdbm;

// Hash "DATA*AttackRange" to get the lookup key
let key = sdbm::hash_lower_with_delimiter("DATA", "AttackRange", '*');
let value = inibin.get(key);
```

## Modifying values

```rust
use ltk_inibin::{Inibin, Value};
use ltk_hash::sdbm;

let mut inibin = Inibin::from_reader(&mut reader)?;

// Insert a new float value
let key = sdbm::hash_lower_with_delimiter("DATA", "AttackRange", '*');
inibin.insert(key, Value::F32(550.0));

// Insert an Int64 value
inibin.insert(0x1234, Value::I64(9999999999));

// Remove a value
inibin.remove(key);
```

## Writing an inibin file

```rust
use std::fs::File;
use std::io::BufWriter;

let file = File::create("output.inibin")?;
let mut writer = BufWriter::new(file);
inibin.to_writer(&mut writer)?;
```

## Round-trip

```rust
use std::io::Cursor;

let inibin = Inibin::from_reader(&mut reader)?;

let mut buf = Vec::new();
inibin.to_writer(&mut buf)?;

let mut cursor = Cursor::new(&buf);
let inibin2 = Inibin::from_reader(&mut cursor)?;
// inibin and inibin2 contain identical data
```

## Iterating all values

```rust
for (key, value) in inibin.iter() {
    println!("0x{:08X} = {:?}", key, value);
}
```

## Accessing a specific set bucket

```rust
use ltk_inibin::ValueFlags;

if let Some(float_section) = inibin.section(ValueFlags::F32_LIST) {
    println!("Float section has {} entries", float_section.len());
    // Use .keys(), .values(), or .iter()
    for key in float_section.keys() {
        println!("  key: 0x{:08X}", key);
    }
    for value in float_section.values() {
        println!("  value: {:?}", value);
    }
}
```

## Resolving hash keys to names (ltk_inibin_names)

```rust
use ltk_inibin_names;

// Look up a known hash key
if let Some((section, name)) = ltk_inibin_names::lookup(0xABCD1234) {
    println!("{section}*{name}");
}

// Combine with inibin iteration for human-readable output
for (key, value) in inibin.iter() {
    match ltk_inibin_names::lookup(key) {
        Some((section, name)) => println!("{section}*{name} = {:?}", value),
        None => println!("0x{:08X} = {:?}", key, value),
    }
}
```
