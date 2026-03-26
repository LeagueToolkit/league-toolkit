//! Read an inibin/troybin file and print all key-value pairs.
//!
//! Usage: cargo run -p ltk_inibin --example read_inibin -- <path>

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
    let inibin = ltk_inibin::Inibin::from_reader(&mut reader).unwrap_or_else(|e| {
        eprintln!("Failed to parse {path}: {e}");
        std::process::exit(1);
    });

    println!("{path}: {} entries", inibin.len());
    println!();

    for (key, value) in inibin.iter() {
        println!("  0x{key:08X} = {value:?}");
    }
}
