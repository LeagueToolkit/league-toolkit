//! INI text reader and writer for troybin data.
//!
//! The INI format uses `[SectionName]` headers and `key=value` lines.
//! An `[UNKNOWN_HASHES]` section holds entries whose hashes could not
//! be resolved to known property names.

use std::collections::HashSet;

use crate::dictionary;
use crate::error::TroybinError;
use crate::hash::section_field_hash;
use crate::types::{Property, RawEntry, Section, StorageType, Troybin, Value};

// ── INI text writer ─────────────────────────────────────────────────────────

/// Format a resolved `Troybin` as INI text.
pub fn write_ini(troybin: &Troybin) -> String {
    let mut output = String::new();

    let mut sections: Vec<&Section> = troybin.sections.iter().collect();
    sections.sort_by(|a, b| a.name.cmp(&b.name));

    for section in &sections {
        output.push_str(&format!("[{}]\r\n", section.name));
        let mut props: Vec<&Property> = section.properties.iter().collect();
        props.sort_by(|a, b| a.name.cmp(&b.name));
        for prop in &props {
            let val = prop.value.to_ini_string();
            output.push_str(&format!("{}={}\r\n", prop.name, val));
        }
        output.push_str("\r\n");
    }

    if !troybin.unknown_entries.is_empty() {
        output.push_str("[UNKNOWN_HASHES]\r\n");
        for entry in &troybin.unknown_entries {
            let val = entry.value.to_ini_string();
            output.push_str(&format!("{}={}\r\n", entry.hash, val));
        }
    }

    output
}

// ── INI text reader ─────────────────────────────────────────────────────────

/// Parse INI text back into a `Troybin` structure.
///
/// This reads `[Section]` headers and `key=value` lines, then hashes each
/// section+field pair to produce `RawEntry` items for binary round-trip.
///
/// The `[UNKNOWN_HASHES]` section is special: keys are raw hash values (decimal).
pub fn read_ini(text: &str) -> Result<Troybin, TroybinError> {
    let mut sections: Vec<Section> = Vec::new();
    let mut unknown_entries: Vec<RawEntry> = Vec::new();
    let mut raw_entries: Vec<RawEntry> = Vec::new();

    let mut current_section: Option<String> = None;
    let mut in_unknown = false;

    for (line_num, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Section header
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let name = &trimmed[1..trimmed.len() - 1];
            if name == "UNKNOWN_HASHES" {
                in_unknown = true;
                current_section = None;
            } else {
                in_unknown = false;
                current_section = Some(name.to_string());
                // Ensure section exists
                if !sections.iter().any(|s| s.name == name) {
                    sections.push(Section {
                        name: name.to_string(),
                        properties: Vec::new(),
                    });
                }
            }
            continue;
        }

        // Key=value line
        let eq_pos = match trimmed.find('=') {
            Some(p) => p,
            None => continue, // skip malformed lines
        };
        let key = trimmed[..eq_pos].trim();
        let val_str = trimmed[eq_pos + 1..].trim();

        if in_unknown {
            // Key is a decimal hash
            let hash: u32 = key.parse().map_err(|_| TroybinError::IniParse {
                line: line_num + 1,
                message: format!("Invalid hash in UNKNOWN_HASHES: '{}'", key),
            })?;
            let value = parse_ini_value(val_str);
            let storage = infer_storage_type(&value);
            let entry = RawEntry {
                hash,
                value,
                storage,
            };
            unknown_entries.push(entry.clone());
            raw_entries.push(entry);
        } else if let Some(ref section_name) = current_section {
            let hash = section_field_hash(section_name, key);
            let value = parse_ini_value(val_str);
            let storage = infer_storage_type(&value);

            let entry = RawEntry {
                hash,
                value: value.clone(),
                storage,
            };
            raw_entries.push(entry);

            let section = sections
                .iter_mut()
                .find(|s| s.name == *section_name)
                .unwrap();
            section.properties.push(Property {
                name: key.to_string(),
                value,
                hash,
                storage,
            });
        }
    }

    Ok(Troybin {
        version: 2,
        sections,
        unknown_entries,
        raw_entries,
    })
}

/// Parse an INI value string into a `Value`.
fn parse_ini_value(s: &str) -> Value {
    // Quoted string
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        return Value::String(s[1..s.len() - 1].to_string());
    }

    // NaN
    if s.eq_ignore_ascii_case("nan") {
        return Value::Float(f64::NAN);
    }

    // Space-separated vector
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() > 1 {
        let nums: Vec<f64> = parts.iter().filter_map(|p| p.parse().ok()).collect();
        if nums.len() == parts.len() {
            return Value::Vec(nums);
        }
    }

    // Integer (no decimal point, no 'e')
    if let Ok(v) = s.parse::<i32>() {
        if !s.contains('.') && !s.contains('e') && !s.contains('E') {
            return Value::Int(v);
        }
    }

    // Float
    if let Ok(v) = s.parse::<f64>() {
        return Value::Float(v);
    }

    // Bare string (unquoted, e.g. texture paths from some files)
    Value::String(s.to_string())
}

/// Infer a reasonable storage type from a value.
///
/// When reading from INI text we don't know the original storage type,
/// so we pick a reasonable default that will round-trip correctly.
fn infer_storage_type(value: &Value) -> StorageType {
    match value {
        Value::Int(_) => StorageType::Int32,
        Value::Float(_) => StorageType::Float32,
        Value::String(_) => StorageType::StringBlock,
        Value::Vec(v) => match v.len() {
            2 => StorageType::Float32x2,
            3 => StorageType::Float32x3,
            4 => StorageType::Float32x4,
            _ => StorageType::Float32,
        },
    }
}

// ── Resolve raw entries into sections ───────────────────────────────────────

/// Resolve raw entries into named sections using the hash dictionary.
///
/// This is the core function that turns flat `RawEntry` hashes into
/// structured `Section`/`Property` data.
pub fn resolve_entries(raw: &[RawEntry]) -> (Vec<Section>, Vec<RawEntry>) {
    let mut sections: Vec<Section> = Vec::new();
    let mut unknown: Vec<RawEntry> = Vec::new();
    let mut found_hashes: HashSet<u32> = HashSet::new();

    // First pass: match dictionary entries in order (preserves priority of
    // first match, same as the JS/C# implementations).
    let group_names = dictionary::extract_group_names(raw);
    let full_map = dictionary::build_hash_map(&group_names);

    // To match the original tools' section ordering, we look up each raw entry in the map.
    for entry in raw {
        if found_hashes.contains(&entry.hash) {
            continue;
        }

        if let Some((section_name, field_name)) = full_map.get(&entry.hash) {
            let prop = Property {
                name: field_name.clone(),
                value: entry.value.clone(),
                hash: entry.hash,
                storage: entry.storage,
            };

            if let Some(section) = sections.iter_mut().find(|s| s.name == *section_name) {
                section.properties.push(prop);
            } else {
                sections.push(Section {
                    name: section_name.clone(),
                    properties: vec![prop],
                });
            }
            found_hashes.insert(entry.hash);
        }
    }

    // Collect unresolved entries
    for entry in raw {
        if !found_hashes.contains(&entry.hash) {
            unknown.push(entry.clone());
            found_hashes.insert(entry.hash); // don't duplicate
        }
    }

    (sections, unknown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_ini_text() {
        let ini = "[System]\r\nGroupPart0=TestEmitter\r\nGroupPart0Type=1\r\n\r\n[TestEmitter]\r\np-type=1\r\np-texture=\"particles/test.dds\"\r\n\r\n";
        let parsed = read_ini(ini).unwrap();
        assert_eq!(parsed.sections.len(), 2);
        assert_eq!(parsed.sections[0].name, "System");
        assert_eq!(parsed.sections[1].name, "TestEmitter");

        // Write back
        let output = write_ini(&parsed);
        // Both sections should be present
        assert!(output.contains("[System]"));
        assert!(output.contains("[TestEmitter]"));
        assert!(output.contains("GroupPart0=\"TestEmitter\""));
        assert!(output.contains("p-texture=\"particles/test.dds\""));
    }

    #[test]
    fn parse_unknown_hashes() {
        let ini = "[UNKNOWN_HASHES]\r\n12345=42\r\n67890=\"mystery.dds\"\r\n";
        let parsed = read_ini(ini).unwrap();
        assert_eq!(parsed.unknown_entries.len(), 2);
        assert_eq!(parsed.unknown_entries[0].hash, 12345);
        match &parsed.unknown_entries[0].value {
            Value::Int(v) => assert_eq!(*v, 42),
            _ => panic!("Expected Int"),
        }
    }

    #[test]
    fn parse_vectors() {
        let ini = "[System]\r\ntest=1.0 2.0 3.0\r\n";
        let parsed = read_ini(ini).unwrap();
        let prop = &parsed.sections[0].properties[0];
        match &prop.value {
            Value::Vec(v) => {
                assert_eq!(v.len(), 3);
                assert_eq!(v[0], 1.0);
            }
            _ => panic!("Expected Vec"),
        }
    }
}
