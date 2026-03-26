//! Create an inibin file from scratch and write it to disk.
//!
//! Usage: cargo run -p ltk_inibin --example create_inibin -- <output_path>

use ltk_inibin::{Inibin, Value};
use std::{fs::File, io::BufWriter};

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: create_inibin <output_path>");
        std::process::exit(1);
    });

    let mut inibin = Inibin::new();

    // Insert values using From<T> convenience
    inibin.insert(0x0001, 42i32);
    inibin.insert(0x0002, 550.0f32);
    inibin.insert(0x0003, "hello world");
    inibin.insert(0x0004, true);
    inibin.insert(0x0005, -100i16);
    inibin.insert(0x0006, 9999999999i64);

    // Insert packed float (raw byte, decodes to byte * 0.1)
    inibin.insert(0x0007, Value::U8(200)); // 200 * 0.1 = 20.0

    // Insert vector types
    inibin.insert(0x0008, Value::Vec3F32(glam::Vec3::new(1.0, 2.0, 3.0)));
    inibin.insert(0x0009, Value::Vec3U8([10, 20, 30]));

    println!("Created inibin with {} entries", inibin.len());

    // Write to file
    let file = File::create(&path).unwrap_or_else(|e| {
        eprintln!("Failed to create {path}: {e}");
        std::process::exit(1);
    });
    let mut writer = BufWriter::new(file);
    inibin.to_writer(&mut writer).unwrap();

    println!("Written to {path}");

    // Demonstrate generic get_as
    println!();
    println!("Reading back with get_as:");
    println!("  i32:    {:?}", inibin.get_as::<i32>(0x0001));
    println!("  f32:    {:?}", inibin.get_as::<f32>(0x0002));
    println!("  string: {:?}", inibin.get_as::<&str>(0x0003));
    println!("  bool:   {:?}", inibin.get_as::<bool>(0x0004));
    println!("  i16:    {:?}", inibin.get_as::<i16>(0x0005));
    println!("  i64:    {:?}", inibin.get_as::<i64>(0x0006));
    println!(
        "  u8 float: {:?}",
        inibin.get(0x0007).and_then(|v| v.u8_as_f32())
    );
}
