//! `ltk_troybin` — Troybin (INIBIN v1/v2) parser and writer for League Toolkit.
//!
//! This crate provides full round-trip support for the `.troybin` particle
//! configuration format used in League of Legends:
//!
//! - **Read** binary `.troybin` files (v1 and v2)
//! - **Write** binary v2 `.troybin` files
//! - **Read/write** INI text representation
//! - **Hash resolution** via the built-in property name dictionary
//!
//! # Quick start
//!
//! ```no_run
//! use ltk_troybin::{read_troybin, write_troybin_binary, write_troybin_ini};
//!
//! // Read a .troybin binary file
//! let data = std::fs::read("particle.troybin").unwrap();
//! let troybin = read_troybin(&data).unwrap();
//!
//! // Convert to INI text
//! let ini_text = write_troybin_ini(&troybin);
//!
//! // Write back to binary (v2)
//! let binary = write_troybin_binary(&troybin).unwrap();
//! ```

pub mod dictionary;
pub mod error;
pub mod hash;
pub mod reader;
pub mod text;
pub mod types;
pub mod writer;

pub use error::TroybinError;
pub use types::{Property, RawEntry, Section, StorageType, Troybin, Value};

/// Read a `.troybin` binary buffer into a resolved `Troybin` structure.
///
/// This parses the binary data, resolves hashes using the built-in dictionary,
/// and returns a structured document with named sections and properties.
pub fn read_troybin(data: &[u8]) -> Result<Troybin, TroybinError> {
    let (version, raw_entries) = reader::read_binary(data)?;
    let (sections, unknown_entries) = text::resolve_entries(&raw_entries);

    Ok(Troybin {
        version,
        sections,
        unknown_entries,
        raw_entries,
    })
}

/// Read a `.troybin` binary buffer into raw entries without hash resolution.
///
/// Useful when you need direct access to the hashed entries or want to
/// perform custom resolution.
pub fn read_troybin_raw(data: &[u8]) -> Result<(u8, Vec<RawEntry>), TroybinError> {
    reader::read_binary(data)
}

/// Write a `Troybin` to binary v2 format.
///
/// Uses the `raw_entries` field for serialization. If you've modified the
/// sections/properties, call [`sync_raw_entries`] first to update raw_entries
/// from the structured data.
pub fn write_troybin_binary(troybin: &Troybin) -> Result<Vec<u8>, TroybinError> {
    writer::write_binary(&troybin.raw_entries)
}

/// Write a `Troybin` to INI text format.
pub fn write_troybin_ini(troybin: &Troybin) -> String {
    text::write_ini(troybin)
}

/// Parse INI text back into a `Troybin` structure.
///
/// The section+field names are hashed to produce `raw_entries` for
/// binary round-trip.
pub fn read_troybin_ini(text: &str) -> Result<Troybin, TroybinError> {
    text::read_ini(text)
}

/// Convert raw binary data directly to INI text (convenience function).
///
/// Equivalent to `read_troybin(data)` → `write_troybin_ini(&result)`.
pub fn convert_troybin(data: &[u8]) -> Result<String, TroybinError> {
    let troybin = read_troybin(data)?;
    Ok(write_troybin_ini(&troybin))
}

/// Sync `raw_entries` from the structured `sections` + `unknown_entries`.
///
/// Call this after modifying sections/properties to update the raw_entries
/// vector before writing to binary.
pub fn sync_raw_entries(troybin: &mut Troybin) {
    let mut raw = Vec::new();
    for section in &troybin.sections {
        for prop in &section.properties {
            raw.push(RawEntry {
                hash: prop.hash,
                value: prop.value.clone(),
                storage: prop.storage,
            });
        }
    }
    raw.extend(troybin.unknown_entries.iter().cloned());
    troybin.raw_entries = raw;
}
