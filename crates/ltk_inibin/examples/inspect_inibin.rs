//! Inspect an inibin file: print summary statistics and query specific keys.
//!
//! Demonstrates:
//! - File summary (entry counts per section type)
//! - Looking up specific keys by SDBM hash
//! - Using `as_*()` unified accessors to decode packed values
//! - Filtering and searching across all entries
//! - Section-level `.keys()` and `.values()` iterators
//!
//! Usage: cargo run -p ltk_inibin --example inspect_inibin -- <path> [section*property ...]

use ltk_inibin::{Inibin, Value, ValueFlags};
use std::{fs::File, io::BufReader};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: inspect_inibin <path> [section*property ...]");
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  inspect_inibin annie.inibin");
        eprintln!("  inspect_inibin annie.inibin DATA*AttackRange DATA*MoveSpeed");
        std::process::exit(1);
    }

    let path = &args[1];
    let queries: Vec<&str> = args[2..].iter().map(|s| s.as_str()).collect();

    // ── Parse ───────────────────────────────────────────────────
    let file = File::open(path).unwrap_or_else(|e| {
        eprintln!("Failed to open {path}: {e}");
        std::process::exit(1);
    });
    let mut reader = BufReader::new(file);
    let inibin = Inibin::from_reader(&mut reader).unwrap_or_else(|e| {
        eprintln!("Failed to parse {path}: {e}");
        std::process::exit(1);
    });

    // ── Summary ─────────────────────────────────────────────────
    println!("File: {path}");
    println!("Total entries: {}", inibin.len());
    println!();

    let section_names = [
        (ValueFlags::INT32_LIST, "Int32"),
        (ValueFlags::F32_LIST, "Float32"),
        (ValueFlags::U8_LIST, "U8 (packed float)"),
        (ValueFlags::INT16_LIST, "Int16"),
        (ValueFlags::INT8_LIST, "Int8"),
        (ValueFlags::BIT_LIST, "Bool"),
        (ValueFlags::VEC3_U8_LIST, "Vec3U8"),
        (ValueFlags::VEC3_F32_LIST, "Vec3F32"),
        (ValueFlags::VEC2_U8_LIST, "Vec2U8"),
        (ValueFlags::VEC2_F32_LIST, "Vec2F32"),
        (ValueFlags::VEC4_U8_LIST, "Vec4U8"),
        (ValueFlags::VEC4_F32_LIST, "Vec4F32"),
        (ValueFlags::STRING_LIST, "String"),
        (ValueFlags::INT64_LIST, "Int64"),
    ];

    println!("Sections:");
    for (flags, name) in &section_names {
        if let Some(section) = inibin.section(*flags) {
            println!("  {name:20} {: >5} entries", section.len());
        }
    }
    println!();

    // ── Key queries ─────────────────────────────────────────────
    if queries.is_empty() {
        println!("Tip: pass section*property pairs as arguments to query specific keys.");
        println!("  e.g.: inspect_inibin {path} DATA*AttackRange");
        return;
    }

    println!("Queries:");
    for query in &queries {
        // Split on '*' to get section and property
        let parts: Vec<&str> = query.splitn(2, '*').collect();
        let (section, property) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            eprintln!("  {query}: invalid format (expected section*property)");
            continue;
        };

        let hash = ltk_hash::sdbm::hash_inibin_key(section, property);
        print!("  {query} (0x{hash:08X}): ");

        match inibin.get(hash) {
            Some(value) => {
                print_decoded(value);
                println!();
            }
            None => println!("not found"),
        }
    }

    // ── Find all string values ──────────────────────────────────
    if let Some(string_section) = inibin.section(ValueFlags::STRING_LIST) {
        println!();
        println!("All string values ({} entries):", string_section.len());
        for (key, value) in string_section.iter() {
            if let Value::String(s) = value {
                println!("  0x{key:08X} = {s:?}");
            }
        }
    }

    // ── Find all float-like values using unified accessors ──────
    println!();
    println!("All float-like values (F32 + packed U8):");
    let mut count = 0;
    for (key, value) in inibin.iter() {
        if let Some(f) = value.as_f32() {
            let kind = if matches!(value, Value::U8(_)) {
                "packed"
            } else {
                "f32"
            };
            println!("  0x{key:08X} = {f:.2} ({kind})");
            count += 1;
            if count >= 20 {
                let remaining = inibin
                    .iter()
                    .filter(|(_, v)| v.as_f32().is_some())
                    .count()
                    - 20;
                if remaining > 0 {
                    println!("  ... and {remaining} more");
                }
                break;
            }
        }
    }
}

fn print_decoded(value: &Value) {
    match value {
        Value::I32(v) => print!("{v} (i32)"),
        Value::F32(v) => print!("{v} (f32)"),
        Value::U8(v) => print!("{} (u8, decoded: {:.1})", v, value.as_f32().unwrap()),
        Value::I16(v) => print!("{v} (i16)"),
        Value::I8(v) => print!("{v} (u8 raw)"),
        Value::Bool(v) => print!("{v}"),
        Value::Vec2U8(_) => {
            let d = value.as_vec2().unwrap();
            print!("[{:.1}, {:.1}] (vec2 packed)", d.x, d.y);
        }
        Value::Vec2F32(v) => print!("[{}, {}] (vec2)", v.x, v.y),
        Value::Vec3U8(_) => {
            let d = value.as_vec3().unwrap();
            print!("[{:.1}, {:.1}, {:.1}] (vec3 packed)", d.x, d.y, d.z);
        }
        Value::Vec3F32(v) => print!("[{}, {}, {}] (vec3)", v.x, v.y, v.z),
        Value::Vec4U8(_) => {
            let d = value.as_vec4().unwrap();
            print!(
                "[{:.1}, {:.1}, {:.1}, {:.1}] (vec4 packed)",
                d.x, d.y, d.z, d.w
            );
        }
        Value::Vec4F32(v) => print!("[{}, {}, {}, {}] (vec4)", v.x, v.y, v.z, v.w),
        Value::String(v) => print!("{v:?}"),
        Value::I64(v) => print!("{v} (i64)"),
    }
}
