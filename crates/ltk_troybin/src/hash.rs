/// The inibin hash function (65599-based rolling hash, case-insensitive).
///
/// This is the same `ihash` used in Leischii's TroybinConverter and Rey's TroybinEditor.
pub fn ihash(value: &str, init: u32) -> u32 {
    let mut ret = init;
    for ch in value.chars() {
        let lower = ch.to_ascii_lowercase() as u32;
        ret = lower.wrapping_add(ret.wrapping_mul(65599));
    }
    ret
}

/// Compute the hash for a section+field pair.
///
/// The hash is: `ihash(field, ihash("*", ihash(section, 0)))`.
pub fn section_field_hash(section: &str, field: &str) -> u32 {
    let section_hash = ihash("*", ihash(section, 0));
    ihash(field, section_hash)
}

/// Compute hashes for all (section, field) combinations, including
/// the commented variant (`'field`).
///
/// Returns `(section, field_name, hash)` tuples.
pub fn build_hash_entries(sections: &[String], names: &[String]) -> Vec<(String, String, u32)> {
    let comments = ["", "'"];
    let mut result = Vec::new();
    for section in sections {
        let section_hash = ihash("*", ihash(section, 0));
        for name in names {
            for c in &comments {
                let name_entry = format!("{}{}", c, name);
                let ret = ihash(&name_entry, section_hash);
                result.push((section.clone(), name_entry, ret));
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ihash_basic() {
        // Known value: ihash("System", 0) should produce a deterministic result
        let h = ihash("System", 0);
        assert_ne!(h, 0);
        // Case insensitive
        assert_eq!(ihash("system", 0), h);
        assert_eq!(ihash("SYSTEM", 0), h);
    }

    #[test]
    fn test_section_field_hash() {
        let h1 = section_field_hash("System", "GroupPart0");
        let h2 = section_field_hash("System", "GroupPart1");
        assert_ne!(h1, h2);
        // Same inputs produce same hash
        assert_eq!(h1, section_field_hash("System", "GroupPart0"));
    }
}
