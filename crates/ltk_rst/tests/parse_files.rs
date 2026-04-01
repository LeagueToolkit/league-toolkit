//! Integration tests for RST parsing using real game files.
//!
//! Test files live at `<workspace-root>/../test-files/data/menu/`.
//! Tests that reference missing files are skipped rather than failing, so the
//! suite can run in CI environments that do not include game assets.

use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::Path;

use ltk_rst::{compute_hash, RstError, RstFile, RstHashType, RstVersion};

const TEST_FILES_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../test-files/data/menu");

fn open(relative: &str) -> Option<BufReader<File>> {
    let path = Path::new(TEST_FILES_ROOT).join(relative);
    if !path.exists() {
        println!("skipping missing file: {}", path.display());
        return None;
    }
    Some(BufReader::new(File::open(&path).unwrap_or_else(|e| {
        panic!("failed to open {}: {e}", path.display())
    })))
}

// ---------------------------------------------------------------------------
// Parse tests
// ---------------------------------------------------------------------------

/// Parses every locale's bootstrap.stringtable to ensure the reader handles
/// all regional encodings (CJK, Arabic, Cyrillic, …) without error.
#[test]
fn parse_all_bootstrap_locales() {
    let locales = [
        "ar_ae", "cs_cz", "de_de", "el_gr", "en_au", "en_gb", "en_ph", "en_sg", "en_us", "es_ar",
        "es_es", "es_mx", "fr_fr", "hu_hu", "id_id", "it_it", "ja_jp", "ko_kr", "pl_pl", "pt_br",
        "ro_ro", "ru_ru", "th_th", "tr_tr", "vi_vn", "zh_cn", "zh_my", "zh_tw",
    ];

    for locale in locales {
        let Some(mut reader) = open(&format!("{locale}/bootstrap.stringtable")) else {
            continue;
        };

        let rst = RstFile::from_reader(&mut reader)
            .unwrap_or_else(|e| panic!("failed to parse {locale}/bootstrap.stringtable: {e}"));

        assert_eq!(rst.version, RstVersion::V5, "{locale}: expected version 5");
        assert!(
            !rst.entries.is_empty(),
            "{locale}: expected at least one entry"
        );

        println!(
            "{locale}/bootstrap.stringtable: {} entries",
            rst.entries.len()
        );
    }
}

/// Parses the large LoL and TFT string tables and checks their entry counts.
#[test]
fn parse_lol_and_tft_stringtables() {
    for (name, expected_count) in [("lol", 115310usize), ("tft", 94652usize)] {
        let Some(mut reader) = open(&format!("en_us/{name}.stringtable")) else {
            continue;
        };

        let rst = RstFile::from_reader(&mut reader)
            .unwrap_or_else(|e| panic!("failed to parse en_us/{name}.stringtable: {e}"));

        assert_eq!(rst.version, RstVersion::V5);
        assert_eq!(
            rst.entries.len(),
            expected_count,
            "{name}.stringtable entry count mismatch"
        );

        println!("en_us/{name}.stringtable: {} entries", rst.entries.len());
    }
}

/// Verifies known hash→string mappings in en_us/bootstrap.stringtable.
#[test]
fn parse_bootstrap_known_entries() {
    let Some(mut reader) = open("en_us/bootstrap.stringtable") else {
        return;
    };

    let rst =
        RstFile::from_reader(&mut reader).expect("failed to parse en_us/bootstrap.stringtable");

    assert_eq!(rst.entries.len(), 201);

    // Known stable entries confirmed from the file.
    assert_eq!(rst.get(0x000000008818cc3c), Some("Ignore"));
    assert_eq!(rst.get(0x0000004732dbee5e), Some("Cancel"));
}

// ---------------------------------------------------------------------------
// Round-trip tests
// ---------------------------------------------------------------------------

/// Parses en_us/bootstrap.stringtable, serialises it back to bytes, parses
/// those bytes again, and asserts the two parsed representations are equal.
#[test]
fn round_trip_bootstrap() {
    let Some(mut reader) = open("en_us/bootstrap.stringtable") else {
        return;
    };

    let original =
        RstFile::from_reader(&mut reader).expect("failed to parse en_us/bootstrap.stringtable");

    let mut buf = Vec::new();
    original
        .to_writer(&mut buf)
        .expect("failed to serialise bootstrap.stringtable");

    let mut cursor = Cursor::new(&buf);
    let reloaded =
        RstFile::from_reader(&mut cursor).expect("failed to re-parse serialised bootstrap");

    assert_eq!(
        original.version, reloaded.version,
        "version mismatch after round-trip"
    );
    assert_eq!(
        original.entries.len(),
        reloaded.entries.len(),
        "entry count mismatch after round-trip"
    );
    for (hash, text) in &original.entries {
        assert_eq!(
            reloaded.get(*hash),
            Some(text.as_str()),
            "entry {hash:#018x} missing or changed after round-trip"
        );
    }
}

// ---------------------------------------------------------------------------
// Hash tests
// ---------------------------------------------------------------------------

/// compute_hash lowercases before hashing, so both cases must produce the same
/// result.
#[test]
fn compute_hash_is_case_insensitive() {
    let lower = compute_hash("game_client_quit", RstHashType::Simple);
    let upper = compute_hash("GAME_CLIENT_QUIT", RstHashType::Simple);
    let mixed = compute_hash("Game_Client_Quit", RstHashType::Simple);

    assert_eq!(lower, upper);
    assert_eq!(lower, mixed);
}

/// The Simple (v4/v5) mask is 39 bits; the Complex (v2/v3) mask is 40 bits.
/// Each hash must fit within its own mask.
#[test]
fn compute_hash_respects_bit_width() {
    let simple_mask = (1u64 << 39) - 1;
    let complex_mask = (1u64 << 40) - 1;

    let simple_hash = compute_hash("some_key", RstHashType::Simple);
    let complex_hash = compute_hash("some_key", RstHashType::Complex);

    assert_eq!(simple_hash & simple_mask, simple_hash);
    assert_eq!(complex_hash & complex_mask, complex_hash);
}

// ---------------------------------------------------------------------------
// Error tests
// ---------------------------------------------------------------------------

#[test]
fn invalid_magic_returns_error() {
    let bad = b"\x00\x00\x00\x05";
    let mut cursor = Cursor::new(bad);
    let err = RstFile::from_reader(&mut cursor).unwrap_err();
    assert!(
        matches!(err, RstError::InvalidMagic { .. }),
        "expected InvalidMagic, got {err:?}"
    );
}

#[test]
fn unsupported_version_returns_error() {
    let bad = b"RST\x01";
    let mut cursor = Cursor::new(bad);
    let err = RstFile::from_reader(&mut cursor).unwrap_err();
    assert!(
        matches!(err, RstError::UnsupportedVersion { version: 0x01 }),
        "expected UnsupportedVersion(0x01), got {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Builder / insertion tests
// ---------------------------------------------------------------------------

/// Verifies that insert_str hashes the key and stores the value, and that the
/// resulting file can be written and re-read with no data loss.
#[test]
fn insert_str_round_trips() {
    let mut rst = RstFile::new(RstVersion::V5);
    rst.insert_str("game_client_quit", "Quit");
    rst.insert_str("game_client_play", "Play");

    let mut buf = Vec::new();
    rst.to_writer(&mut buf).expect("serialise failed");

    let mut cursor = Cursor::new(&buf);
    let loaded = RstFile::from_reader(&mut cursor).expect("re-parse failed");

    let quit_hash = compute_hash("game_client_quit", RstHashType::Simple);
    let play_hash = compute_hash("game_client_play", RstHashType::Simple);

    assert_eq!(loaded.get(quit_hash), Some("Quit"));
    assert_eq!(loaded.get(play_hash), Some("Play"));
}

/// Entries with identical string values must share a single copy in the
/// serialised byte stream.
#[test]
fn to_writer_deduplicates_strings() {
    let mut rst = RstFile::new(RstVersion::V5);
    let shared_value = "Shared string value";

    for i in 0u64..10 {
        rst.insert(i, shared_value);
    }

    let mut buf = Vec::new();
    rst.to_writer(&mut buf).expect("serialise failed");

    let occurrences = buf
        .windows(shared_value.len())
        .filter(|w| *w == shared_value.as_bytes())
        .count();

    assert_eq!(
        occurrences, 1,
        "string should appear exactly once in output"
    );
}
