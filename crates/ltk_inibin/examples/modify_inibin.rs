//! Read an inibin file, modify values, and write the result.
//!
//! Demonstrates:
//! - Reading and modifying an existing inibin file
//! - Inserting, updating, and removing values by hash key
//! - Cross-bucket migration (changing a value's type)
//! - Using `contains_key` and `get_or` for safe access
//! - Section-level access with `section()` and `section_mut()`
//! - Hashing section/property keys with `ltk_hash::sdbm`
//!
//! Usage: cargo run -p ltk_inibin --example modify_inibin -- <input_path> <output_path>

use ltk_inibin::{Inibin, ValueFlags};
use std::{
    fs::File,
    io::{BufReader, BufWriter},
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: modify_inibin <input_path> <output_path>");
        std::process::exit(1);
    }
    let input_path = &args[1];
    let output_path = &args[2];

    // ── Read ────────────────────────────────────────────────────
    let file = File::open(input_path).unwrap_or_else(|e| {
        eprintln!("Failed to open {input_path}: {e}");
        std::process::exit(1);
    });
    let mut reader = BufReader::new(file);
    let mut inibin = Inibin::from_reader(&mut reader).unwrap_or_else(|e| {
        eprintln!("Failed to parse {input_path}: {e}");
        std::process::exit(1);
    });

    println!("Read {input_path}: {} entries", inibin.len());

    // ── Insert new values ───────────────────────────────────────
    let key = ltk_hash::sdbm::hash_inibin_key("DATA", "AttackRange");
    let old = inibin.get_or(key, 0.0f32);
    inibin.insert(key, 625.0f32);
    println!("Updated AttackRange: {old} -> 625.0");

    // ── Safe access patterns ────────────────────────────────────
    let move_speed_key = ltk_hash::sdbm::hash_inibin_key("DATA", "MoveSpeed");
    if inibin.contains_key(move_speed_key) {
        println!("MoveSpeed exists: {:?}", inibin.get(move_speed_key));
    } else {
        println!("MoveSpeed not found, inserting default");
        inibin.insert(move_speed_key, 345.0f32);
    }

    // get_or returns a default on missing key or type mismatch
    let armor: f32 = inibin.get_or(0xDEAD, 30.0f32);
    println!("Armor (or default): {armor}");

    // ── Cross-bucket migration ──────────────────────────────────
    // Inserting with a different type automatically moves the entry
    inibin.insert(0xFF01, 42i32);
    println!("Inserted 0xFF01 as i32");
    inibin.insert(0xFF01, 42.0f32);
    println!("Re-inserted 0xFF01 as f32 (migrated from Int32 -> Float32 section)");

    // Verify it moved
    assert!(inibin
        .section(ValueFlags::INT32_LIST)
        .and_then(|s| s.get(0xFF01))
        .is_none());
    assert!(inibin
        .section(ValueFlags::F32_LIST)
        .and_then(|s| s.get(0xFF01))
        .is_some());

    // ── Remove a value ──────────────────────────────────────────
    if let Some(removed) = inibin.remove(0xFF01) {
        println!("Removed 0xFF01: {removed:?}");
    }

    // ── Section-level inspection ────────────────────────────────
    if let Some(float_section) = inibin.section(ValueFlags::F32_LIST) {
        println!(
            "\nFloat32 section: {} entries",
            float_section.len()
        );
        for (key, value) in float_section.iter().take(5) {
            println!("  0x{key:08X} = {value:?}");
        }
        if float_section.len() > 5 {
            println!("  ... and {} more", float_section.len() - 5);
        }
    }

    // ── Write ───────────────────────────────────────────────────
    let file = File::create(output_path).unwrap_or_else(|e| {
        eprintln!("Failed to create {output_path}: {e}");
        std::process::exit(1);
    });
    let mut writer = BufWriter::new(file);
    inibin.to_writer(&mut writer).unwrap();

    println!("\nWritten {output_path}: {} entries", inibin.len());
}
