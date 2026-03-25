use ltk_troybin::{
    convert_troybin, read_troybin, read_troybin_ini, read_troybin_raw, sync_raw_entries,
    write_troybin_binary, write_troybin_ini,
};

const FIXTURE: &[u8] = include_bytes!("fixtures/slime_environmentminion_idle.troybin");

#[test]
fn read_real_troybin() {
    let troybin = read_troybin(FIXTURE).unwrap();

    // Should have parsed successfully with sections
    assert!(!troybin.sections.is_empty(), "Expected resolved sections");
    assert!(!troybin.raw_entries.is_empty(), "Expected raw entries");

    // Print summary for manual inspection
    println!("Version: {}", troybin.version);
    println!("Sections: {}", troybin.sections.len());
    for section in &troybin.sections {
        println!(
            "  [{}] — {} properties",
            section.name,
            section.properties.len()
        );
    }
    println!("Unknown entries: {}", troybin.unknown_entries.len());
    println!("Total raw entries: {}", troybin.raw_entries.len());
}

#[test]
fn round_trip_binary() {
    // Read original
    let (version, original_entries) = read_troybin_raw(FIXTURE).unwrap();
    assert_eq!(version, 2);

    // Write to binary
    let mut output = Vec::new();
    let troybin = read_troybin(FIXTURE).unwrap();
    write_troybin_binary(&mut output, &troybin).unwrap();

    // Read back
    let (version2, roundtrip_entries) = read_troybin_raw(&output).unwrap();
    assert_eq!(version2, 2);

    // Same number of entries (minus any OldFormat)
    let non_old: Vec<_> = original_entries
        .iter()
        .filter(|e| e.storage != ltk_troybin::StorageType::OldFormat)
        .collect();
    assert_eq!(
        non_old.len(),
        roundtrip_entries.len(),
        "Entry count mismatch after round-trip"
    );

    // Each hash should match
    for (orig, rt) in non_old.iter().zip(roundtrip_entries.iter()) {
        assert_eq!(orig.hash, rt.hash, "Hash mismatch");
        assert_eq!(orig.storage, rt.storage, "Storage type mismatch");
    }
}

#[test]
fn round_trip_ini_text() {
    // Read binary → INI text
    let troybin = read_troybin(FIXTURE).unwrap();
    let ini = write_troybin_ini(&troybin);

    // INI should contain section headers
    assert!(ini.contains('['), "INI should contain section headers");

    // Parse INI back
    let mut parsed = read_troybin_ini(&ini).unwrap();
    assert_eq!(parsed.sections.len(), troybin.sections.len());

    // Write parsed INI back to binary
    sync_raw_entries(&mut parsed);
    let mut binary_output = Vec::new();
    write_troybin_binary(&mut binary_output, &parsed).unwrap();

    // Should produce valid binary
    let re_read = read_troybin(&binary_output).unwrap();
    assert!(!re_read.sections.is_empty());
}

#[test]
fn convert_convenience() {
    let text = convert_troybin(FIXTURE).unwrap();
    assert!(!text.is_empty());
    assert!(text.contains('['));
}
