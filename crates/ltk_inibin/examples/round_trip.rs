//! Round-trip an inibin file: read -> write -> read, then verify equality.
//!
//! Demonstrates:
//! - Full round-trip workflow (parse → serialize → re-parse → compare)
//! - Writing to an in-memory buffer with `to_writer`
//! - Comparing two `Inibin` instances entry-by-entry
//!
//! Usage: cargo run -p ltk_inibin --example round_trip -- <path>

use std::{fs::File, io::BufReader};

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: round_trip <path>");
        std::process::exit(1);
    });

    let file = File::open(&path).unwrap_or_else(|e| {
        eprintln!("Failed to open {path}: {e}");
        std::process::exit(1);
    });

    let mut reader = BufReader::new(file);
    let original = ltk_inibin::Inibin::from_reader(&mut reader).unwrap_or_else(|e| {
        eprintln!("Failed to parse {path}: {e}");
        std::process::exit(1);
    });

    println!("Read {path}: {} entries", original.len());

    // Write to memory
    let mut buf = Vec::new();
    original.to_writer(&mut buf).unwrap();
    println!("Written: {} bytes", buf.len());

    // Read back
    let mut cursor = std::io::Cursor::new(&buf);
    let roundtripped = ltk_inibin::Inibin::from_reader(&mut cursor).unwrap();
    println!("Re-read: {} entries", roundtripped.len());

    // Compare
    let mut mismatches = 0;
    for (key, value) in original.iter() {
        match roundtripped.get(key) {
            Some(rt_value) if rt_value == value => {}
            Some(rt_value) => {
                println!("  MISMATCH 0x{key:08X}: {value:?} vs {rt_value:?}");
                mismatches += 1;
            }
            None => {
                println!("  MISSING  0x{key:08X}: {value:?}");
                mismatches += 1;
            }
        }
    }

    // Check for entries in roundtripped that are not in original
    for (key, _) in roundtripped.iter() {
        if original.get(key).is_none() {
            println!("  EXTRA    0x{key:08X}");
            mismatches += 1;
        }
    }

    if mismatches == 0 {
        println!("Round-trip OK!");
    } else {
        println!("{mismatches} mismatches found");
        std::process::exit(1);
    }
}
