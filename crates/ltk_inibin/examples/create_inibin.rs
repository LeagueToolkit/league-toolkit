//! Create an inibin file from scratch and write it to disk.
//!
//! Demonstrates:
//! - Constructing an empty `Inibin` and inserting all value types
//! - Using `From<T>` convenience for common types (i32, f32, &str, etc.)
//! - Using `Value::*` variants for packed/vector types
//! - Computing hash keys with `ltk_hash::sdbm`
//! - Writing to a file with `to_writer`
//! - Reading back values with `get_as` and `as_*()` unified accessors
//!
//! Usage: cargo run -p ltk_inibin --example create_inibin -- <output_path>

use glam::{Vec2, Vec3, Vec4};
use ltk_inibin::{Inibin, Value};
use std::{fs::File, io::BufWriter};

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: create_inibin <output_path>");
        std::process::exit(1);
    });

    let mut inibin = Inibin::new();

    // ── Scalar types (From<T> convenience) ──────────────────────
    inibin.insert(0x0001, 42i32);
    inibin.insert(0x0002, 550.0f32);
    inibin.insert(0x0003, "hello world");
    inibin.insert(0x0004, true);
    inibin.insert(0x0005, -100i16);
    inibin.insert(0x0006, 9999999999i64);

    // ── Packed U8 types (raw bytes, decode with as_*()) ─────────
    inibin.insert(0x0100, Value::U8(200)); // as_f32() → 20.0
    inibin.insert(0x0101, Value::Vec2U8([50, 100])); // as_vec2() → (5.0, 10.0)
    inibin.insert(0x0102, Value::Vec3U8([10, 20, 30])); // as_vec3() → (1.0, 2.0, 3.0)
    inibin.insert(0x0103, Value::Vec4U8([25, 50, 75, 100])); // as_vec4() → (2.5, 5.0, 7.5, 10.0)

    // ── F32 vector types ────────────────────────────────────────
    inibin.insert(0x0200, Value::Vec2F32(Vec2::new(1.5, 2.5)));
    inibin.insert(0x0201, Value::Vec3F32(Vec3::new(1.0, 2.0, 3.0)));
    inibin.insert(0x0202, Value::Vec4F32(Vec4::new(1.1, 2.2, 3.3, 4.4)));

    // ── Using SDBM hash for real inibin keys ────────────────────
    let attack_range_key = ltk_hash::sdbm::hash_inibin_key("DATA", "AttackRange");
    inibin.insert(attack_range_key, 550.0f32);

    // String keys work too
    let name = String::from("DATA");
    let prop = String::from("MoveSpeed");
    let move_speed_key = ltk_hash::sdbm::hash_inibin_key(name, prop);
    inibin.insert(move_speed_key, 345.0f32);

    println!("Created inibin with {} entries", inibin.len());

    // ── Write to file ───────────────────────────────────────────
    let file = File::create(&path).unwrap_or_else(|e| {
        eprintln!("Failed to create {path}: {e}");
        std::process::exit(1);
    });
    let mut writer = BufWriter::new(file);
    inibin.to_writer(&mut writer).unwrap();
    println!("Written to {path}");

    // ── Reading values back ─────────────────────────────────────
    println!();
    println!("Typed access with get_as:");
    println!("  i32:    {:?}", inibin.get_as::<i32>(0x0001));
    println!("  f32:    {:?}", inibin.get_as::<f32>(0x0002));
    println!("  string: {:?}", inibin.get_as::<&str>(0x0003));
    println!("  bool:   {:?}", inibin.get_as::<bool>(0x0004));
    println!("  i16:    {:?}", inibin.get_as::<i16>(0x0005));
    println!("  i64:    {:?}", inibin.get_as::<i64>(0x0006));

    // get_or returns a default on missing key or type mismatch
    println!("  missing with default: {}", inibin.get_or(0x9999, 0i32));

    // ── Unified as_*() accessors for packed/non-packed floats ───
    println!();
    println!("Unified as_*() accessors:");

    // as_f32() works on both F32 and U8 variants
    println!(
        "  F32 value:     {:?}",
        inibin.get(0x0002).and_then(|v| v.as_f32())
    );
    println!(
        "  U8 packed:     {:?}",
        inibin.get(0x0100).and_then(|v| v.as_f32())
    );

    // as_vec2() works on both Vec2F32 and Vec2U8 variants
    println!(
        "  Vec2F32 value: {:?}",
        inibin.get(0x0200).and_then(|v| v.as_vec2())
    );
    println!(
        "  Vec2U8 packed: {:?}",
        inibin.get(0x0101).and_then(|v| v.as_vec2())
    );

    // as_vec3() works on both Vec3F32 and Vec3U8 variants
    println!(
        "  Vec3F32 value: {:?}",
        inibin.get(0x0201).and_then(|v| v.as_vec3())
    );
    println!(
        "  Vec3U8 packed: {:?}",
        inibin.get(0x0102).and_then(|v| v.as_vec3())
    );

    // as_vec4() works on both Vec4F32 and Vec4U8 variants
    println!(
        "  Vec4F32 value: {:?}",
        inibin.get(0x0202).and_then(|v| v.as_vec4())
    );
    println!(
        "  Vec4U8 packed: {:?}",
        inibin.get(0x0103).and_then(|v| v.as_vec4())
    );
}
