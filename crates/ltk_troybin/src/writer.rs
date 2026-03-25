//! Binary writer for INIBIN v2 troybin files.
//!
//! Supports full re-serialization of all value types, producing a valid v2
//! binary from a list of `RawEntry` items. Round-trips correctly with the
//! reader when storage types are preserved.

use std::io::Write;

use byteorder::{WriteBytesExt, LE};

use crate::error::TroybinError;
use crate::types::{RawEntry, StorageType, Value};

/// Write a list of raw entries to a v2 troybin binary.
pub fn write_v2<W: Write>(w: &mut W, entries: &[RawEntry]) -> Result<(), TroybinError> {
    // Group entries by storage type (bit index)
    let mut buckets: Vec<Vec<&RawEntry>> = vec![Vec::new(); 14];
    for entry in entries {
        if entry.storage == StorageType::OldFormat {
            // V1 entries can't be written as v2 — skip
            continue;
        }
        let bit: u8 = entry.storage.into();
        if (bit as usize) < 14 {
            buckets[bit as usize].push(entry);
        }
    }

    // Compute flags
    let mut flags: u16 = 0;
    for (bit, bucket) in buckets.iter().enumerate() {
        if !bucket.is_empty() {
            flags |= 1 << bit;
        }
    }

    // Build string pool first (needed for stringsLength header)
    let string_pool = if !buckets[12].is_empty() {
        build_string_pool(&buckets[12])
    } else {
        StringPool {
            offsets: Vec::new(),
            data: Vec::new(),
        }
    };
    let strings_length = string_pool.data.len() as u16;

    // Version byte
    w.write_u8(2)?;

    // stringsLength (u16 LE)
    w.write_u16::<LE>(strings_length)?;

    // flags (u16 LE)
    w.write_u16::<LE>(flags)?;

    // Write each block in flag-bit order
    for bit in 0u8..14 {
        if flags & (1 << bit) == 0 {
            continue;
        }
        let storage = StorageType::try_from(bit)?;
        match bit {
            5 => write_bool_block(w, &buckets[5])?,
            12 => write_string_block(w, &buckets[12], &string_pool)?,
            _ => write_number_block(w, &buckets[bit as usize], storage)?,
        }
    }

    Ok(())
}

// ── Bool block ──────────────────────────────────────────────────────────────

fn write_bool_block<W: Write>(w: &mut W, entries: &[&RawEntry]) -> Result<(), TroybinError> {
    w.write_u16::<LE>(entries.len() as u16)?;

    // Hashes
    for e in entries {
        w.write_u32::<LE>(e.hash)?;
    }

    // Packed booleans
    let bytes_count = entries.len() / 8
        + if !entries.len().is_multiple_of(8) {
            1
        } else {
            0
        };
    let mut packed = vec![0u8; bytes_count];
    for (j, e) in entries.iter().enumerate() {
        let val = match &e.value {
            Value::Int(v) => *v != 0,
            Value::Float(v) => *v != 0.0,
            _ => false,
        };
        if val {
            packed[j / 8] |= 1 << (j % 8);
        }
    }
    w.write_all(&packed)?;
    Ok(())
}

// ── Number block ────────────────────────────────────────────────────────────

fn write_number_block<W: Write>(
    w: &mut W,
    entries: &[&RawEntry],
    storage: StorageType,
) -> Result<(), TroybinError> {
    w.write_u16::<LE>(entries.len() as u16)?;

    // Hashes
    for e in entries {
        w.write_u32::<LE>(e.hash)?;
    }

    let count = storage.component_count();
    let mul = storage.multiplier();

    // Values
    for e in entries {
        let vals = value_to_components(&e.value, count);
        for v in &vals {
            let raw = if mul != 0.0 { v / mul } else { *v };
            write_component(w, raw, storage)?;
        }
    }
    Ok(())
}

fn value_to_components(value: &Value, count: usize) -> Vec<f64> {
    match value {
        Value::Int(v) => vec![*v as f64],
        Value::Float(v) => vec![*v],
        Value::Vec(v) => {
            let mut result = v.clone();
            result.resize(count, 0.0);
            result
        }
        Value::String(_) => vec![0.0; count],
    }
}

fn write_component<W: Write>(
    w: &mut W,
    raw: f64,
    storage: StorageType,
) -> Result<(), TroybinError> {
    match storage {
        StorageType::Int32 | StorageType::Int32Long => {
            w.write_i32::<LE>(raw as i32)?;
        }
        StorageType::Float32
        | StorageType::Float32x2
        | StorageType::Float32x3
        | StorageType::Float32x4 => {
            w.write_f32::<LE>(raw as f32)?;
        }
        StorageType::U8Scaled
        | StorageType::U8
        | StorageType::U8x2Scaled
        | StorageType::U8x3Scaled
        | StorageType::U8x4Scaled => {
            let clamped = raw.round().clamp(0.0, 255.0) as u8;
            w.write_u8(clamped)?;
        }
        StorageType::Int16 => {
            w.write_i16::<LE>(raw as i16)?;
        }
        _ => {}
    }
    Ok(())
}

// ── String block ────────────────────────────────────────────────────────────

struct StringPool {
    offsets: Vec<u16>,
    data: Vec<u8>,
}

fn build_string_pool(entries: &[&RawEntry]) -> StringPool {
    let mut offsets = Vec::with_capacity(entries.len());
    let mut data = Vec::new();

    for e in entries {
        offsets.push(data.len() as u16);
        let s = match &e.value {
            Value::String(s) => s.as_bytes(),
            _ => b"",
        };
        data.extend_from_slice(s);
        data.push(0); // null terminator
    }

    StringPool { offsets, data }
}

fn write_string_block<W: Write>(
    w: &mut W,
    entries: &[&RawEntry],
    pool: &StringPool,
) -> Result<(), TroybinError> {
    w.write_u16::<LE>(entries.len() as u16)?;

    // Hashes
    for e in entries {
        w.write_u32::<LE>(e.hash)?;
    }

    // Offsets (u16 each)
    for &offset in &pool.offsets {
        w.write_u16::<LE>(offset)?;
    }

    // String data
    w.write_all(&pool.data)?;
    Ok(())
}

/// Write entries to binary, choosing version automatically.
///
/// If all entries have `OldFormat` storage, returns an error (v1 writing is
/// not supported — v1 is a legacy read-only format). Otherwise writes v2.
pub fn write_binary<W: Write>(w: &mut W, entries: &[RawEntry]) -> Result<(), TroybinError> {
    let all_old = entries.iter().all(|e| e.storage == StorageType::OldFormat);
    if all_old && !entries.is_empty() {
        return Err(TroybinError::V1WriteNotSupported);
    }
    write_v2(w, entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::read_binary;
    use std::io::Cursor;

    fn write_to_vec(entries: &[RawEntry]) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        write_v2(&mut buf, entries).unwrap();
        buf.into_inner()
    }

    #[test]
    fn round_trip_numbers() {
        let entries = vec![
            RawEntry {
                hash: 100,
                value: Value::Int(42),
                storage: StorageType::Int32,
            },
            RawEntry {
                hash: 200,
                value: Value::Float(2.78),
                storage: StorageType::Float32,
            },
            RawEntry {
                hash: 300,
                value: Value::Vec(vec![1.0, 2.0, 3.0]),
                storage: StorageType::Float32x3,
            },
        ];
        let bytes = write_to_vec(&entries);
        let (version, read_back) = read_binary(&bytes).unwrap();
        assert_eq!(version, 2);
        assert_eq!(read_back.len(), 3);
        assert_eq!(read_back[0].hash, 100);
        assert_eq!(read_back[1].hash, 200);
        assert_eq!(read_back[2].hash, 300);

        // Check values
        match &read_back[0].value {
            Value::Int(v) => assert_eq!(*v, 42),
            _ => panic!("Expected Int"),
        }
    }

    #[test]
    fn round_trip_strings() {
        let entries = vec![
            RawEntry {
                hash: 400,
                value: Value::String("hello.dds".to_string()),
                storage: StorageType::StringBlock,
            },
            RawEntry {
                hash: 500,
                value: Value::String("world.png".to_string()),
                storage: StorageType::StringBlock,
            },
        ];
        let bytes = write_to_vec(&entries);
        let (_, read_back) = read_binary(&bytes).unwrap();
        assert_eq!(read_back.len(), 2);
        match &read_back[0].value {
            Value::String(s) => assert_eq!(s, "hello.dds"),
            _ => panic!("Expected String"),
        }
        match &read_back[1].value {
            Value::String(s) => assert_eq!(s, "world.png"),
            _ => panic!("Expected String"),
        }
    }

    #[test]
    fn round_trip_bools() {
        let entries = vec![
            RawEntry {
                hash: 600,
                value: Value::Int(1),
                storage: StorageType::Bool,
            },
            RawEntry {
                hash: 700,
                value: Value::Int(0),
                storage: StorageType::Bool,
            },
            RawEntry {
                hash: 800,
                value: Value::Int(1),
                storage: StorageType::Bool,
            },
        ];
        let bytes = write_to_vec(&entries);
        let (_, read_back) = read_binary(&bytes).unwrap();
        assert_eq!(read_back.len(), 3);
        match &read_back[0].value {
            Value::Int(v) => assert_eq!(*v, 1),
            _ => panic!(),
        }
        match &read_back[1].value {
            Value::Int(v) => assert_eq!(*v, 0),
            _ => panic!(),
        }
        match &read_back[2].value {
            Value::Int(v) => assert_eq!(*v, 1),
            _ => panic!(),
        }
    }

    #[test]
    fn round_trip_mixed() {
        let entries = vec![
            RawEntry {
                hash: 10,
                value: Value::Int(7),
                storage: StorageType::Int32,
            },
            RawEntry {
                hash: 20,
                value: Value::Float(1.5),
                storage: StorageType::Float32,
            },
            RawEntry {
                hash: 30,
                value: Value::Int(1),
                storage: StorageType::Bool,
            },
            RawEntry {
                hash: 40,
                value: Value::String("test.dds".to_string()),
                storage: StorageType::StringBlock,
            },
            RawEntry {
                hash: 50,
                value: Value::Vec(vec![0.5, 0.6]),
                storage: StorageType::Float32x2,
            },
        ];
        let bytes = write_to_vec(&entries);
        let (version, read_back) = read_binary(&bytes).unwrap();
        assert_eq!(version, 2);
        assert_eq!(read_back.len(), 5);
    }
}
