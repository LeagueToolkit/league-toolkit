//! Read an inibin/troybin file and print all key-value pairs, grouped by section.
//!
//! Demonstrates:
//! - Parsing with `Inibin::from_reader`
//! - Iterating all entries with `inibin.iter()`
//! - Accessing individual sections with `inibin.section()`
//! - Using section `.keys()`, `.values()`, `.iter()` iterators
//! - Using `Value::flags()` to inspect value types
//! - Using unified `as_*()` accessors for decoded output
//!
//! Usage: cargo run -p ltk_inibin --example read_inibin -- <path>

use ltk_inibin::{Inibin, Value, ValueFlags};
use std::{fs::File, io::BufReader};

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: read_inibin <path>");
        std::process::exit(1);
    });

    let file = File::open(&path).unwrap_or_else(|e| {
        eprintln!("Failed to open {path}: {e}");
        std::process::exit(1);
    });

    let mut reader = BufReader::new(file);
    let inibin = Inibin::from_reader(&mut reader).unwrap_or_else(|e| {
        eprintln!("Failed to parse {path}: {e}");
        std::process::exit(1);
    });

    println!("{path}: {} total entries", inibin.len());
    println!();

    // ── Per-section summary ─────────────────────────────────────
    let section_types = [
        (ValueFlags::INT32_LIST, "Int32"),
        (ValueFlags::F32_LIST, "Float32"),
        (ValueFlags::U8_LIST, "U8 (packed float)"),
        (ValueFlags::INT16_LIST, "Int16"),
        (ValueFlags::INT8_LIST, "Int8"),
        (ValueFlags::BIT_LIST, "BitList (bool)"),
        (ValueFlags::VEC3_U8_LIST, "Vec3U8 (packed)"),
        (ValueFlags::VEC3_F32_LIST, "Vec3F32"),
        (ValueFlags::VEC2_U8_LIST, "Vec2U8 (packed)"),
        (ValueFlags::VEC2_F32_LIST, "Vec2F32"),
        (ValueFlags::VEC4_U8_LIST, "Vec4U8 (packed)"),
        (ValueFlags::VEC4_F32_LIST, "Vec4F32"),
        (ValueFlags::STRING_LIST, "String"),
        (ValueFlags::INT64_LIST, "Int64"),
    ];

    for (flags, name) in &section_types {
        if let Some(section) = inibin.section(*flags) {
            println!("── {name} ({} entries) ──", section.len());

            for (key, value) in section.iter() {
                print!("  0x{key:08X} = ");
                print_value(value);
                println!();
            }
            println!();
        }
    }

    // ── Demonstrate .keys() / .values() ─────────────────────────
    if let Some(section) = inibin.section(ValueFlags::F32_LIST) {
        println!("── Float32 keys only ──");
        for key in section.keys() {
            println!("  0x{key:08X}");
        }
        println!();
    }
}

fn print_value(value: &Value) {
    match value {
        Value::I32(v) => print!("{v}"),
        Value::F32(v) => print!("{v}"),
        Value::U8(v) => print!("{v} (decoded: {:.1})", *v as f32 * 0.1),
        Value::I16(v) => print!("{v}"),
        Value::I8(v) => print!("{v}"),
        Value::Bool(v) => print!("{v}"),
        Value::Vec3U8(v) => {
            let decoded = value.as_vec3().unwrap();
            print!("{v:?} (decoded: [{:.1}, {:.1}, {:.1}])", decoded.x, decoded.y, decoded.z);
        }
        Value::Vec3F32(v) => print!("[{}, {}, {}]", v.x, v.y, v.z),
        Value::Vec2U8(v) => {
            let decoded = value.as_vec2().unwrap();
            print!("{v:?} (decoded: [{:.1}, {:.1}])", decoded.x, decoded.y);
        }
        Value::Vec2F32(v) => print!("[{}, {}]", v.x, v.y),
        Value::Vec4U8(v) => {
            let decoded = value.as_vec4().unwrap();
            print!(
                "{v:?} (decoded: [{:.1}, {:.1}, {:.1}, {:.1}])",
                decoded.x, decoded.y, decoded.z, decoded.w
            );
        }
        Value::Vec4F32(v) => print!("[{}, {}, {}, {}]", v.x, v.y, v.z, v.w),
        Value::String(v) => print!("{v:?}"),
        Value::I64(v) => print!("{v}"),
    }
}
