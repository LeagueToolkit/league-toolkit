//! Integration tests for RST parsing.
//!
//! Real-file fixtures live in `tests/rst/` and are committed, so every test
//! here runs in CI without external game assets.

use std::io::Cursor;
use std::path::Path;

use ltk_rst::{HashBits, RstError, RstFormat, RstHash, RstHashAlgo, RstVersion, Stringtable};

const RST_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rst");

fn read_fixture(name: &str) -> Vec<u8> {
    let path = Path::new(RST_DIR).join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

/// Builds a synthetic RST file (no font config, no encrypted entries) packed at
/// a chosen `hash_bits`, so we can exercise the reader/auto-detector directly.
fn build_rst(version: u8, hash_bits: u8, entries: &[(u64, &str)]) -> Vec<u8> {
    let mask = (1u64 << hash_bits) - 1;
    let mut blob: Vec<u8> = Vec::new();
    let mut offsets: std::collections::HashMap<&str, u64> = std::collections::HashMap::new();
    let mut packed: Vec<u64> = Vec::new();

    for (hash, text) in entries {
        let off = *offsets.entry(text).or_insert_with(|| {
            let o = blob.len() as u64;
            blob.extend_from_slice(text.as_bytes());
            blob.push(0);
            o
        });
        packed.push((hash & mask) | (off << hash_bits));
    }

    let mut out = Vec::new();
    out.extend_from_slice(b"RST");
    out.push(version);
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for entry in &packed {
        out.extend_from_slice(&entry.to_le_bytes());
    }
    if version < 5 {
        out.push(0); // encryption-flag byte
    }
    out.extend_from_slice(&blob);
    out
}

// ---------------------------------------------------------------------------
// Synthetic tests (always run)
// ---------------------------------------------------------------------------

/// The reader auto-detects the 38-vs-39 v4/v5 split from the data and resolves
/// strings correctly either way.
#[test]
fn auto_detects_v5_hash_bits() {
    let entries = [
        (0x1111u64, "alpha"),
        (0x2222, "beta"),
        (0x3333, "gamma"),
        (0x4444, "delta"),
    ];

    for bits in [HashBits::B38, HashBits::B39] {
        let bytes = build_rst(5, bits.get(), &entries);
        let table = Stringtable::reader()
            .detect_hash_bits()
            .read(&mut Cursor::new(bytes))
            .expect("failed to parse synthetic v5");

        assert_eq!(
            table.format().hash_bits,
            bits,
            "auto-detect picked the wrong hash_bits"
        );
        let mask = (1u64 << bits.get()) - 1;
        for (hash, text) in entries {
            assert_eq!(table.get(hash & mask), Some(text));
        }
    }
}

/// When both candidate widths fit the entry layout but decode entries
/// differently, detection refuses to guess and errors out.
#[test]
fn detect_rejects_ambiguous_hash_bits() {
    // blob "\0a\0" has entry starts {0, 1, 3}.  This entry decodes to offset 1
    // ("a") under 38 bits but offset 0 ("") under 39 bits — both are valid
    // layouts, so the file is genuinely ambiguous.
    let entry = (1u64 << 38) | 0x1234;
    let mut out = Vec::new();
    out.extend_from_slice(b"RST");
    out.push(5);
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&entry.to_le_bytes());
    out.extend_from_slice(b"\0a\0");

    let err = Stringtable::reader()
        .detect_hash_bits()
        .read(&mut Cursor::new(out))
        .unwrap_err();
    assert!(
        matches!(err, RstError::IndeterminateHashBits { .. }),
        "got {err:?}"
    );
}

/// A tie where both widths decode every entry identically (all raw values fit
/// in 38 bits) is harmless: detection settles on the newer 38.
#[test]
fn detect_resolves_harmless_tie_to_38() {
    let bytes = build_rst(5, 38, &[(0x1234u64, "hello")]);
    let table = Stringtable::reader()
        .detect_hash_bits()
        .read(&mut Cursor::new(bytes))
        .unwrap();
    assert_eq!(table.format().hash_bits, HashBits::B38);
    assert_eq!(table.get(0x1234u64), Some("hello"));
}

/// Pinned hash bits override the default (which assumes the latest, 38-bit): a
/// 39-bit file reads correctly when pinned, with no detection involved.  A
/// pinned algorithm lands in the resulting format too.
#[test]
fn pinned_reader_overrides_defaults() {
    let entries = [(0x1111u64, "alpha"), (0x2222, "beta"), (0x3333, "gamma")];
    let bytes = build_rst(5, 39, &entries);

    let table = Stringtable::reader()
        .hash_bits(HashBits::B39)
        .hash_algo(RstHashAlgo::Xxh64)
        .read(&mut Cursor::new(bytes))
        .expect("failed to parse with pinned format");

    assert_eq!(
        table.format(),
        RstFormat::new(RstVersion::V5, HashBits::B39, RstHashAlgo::Xxh64)
    );
    assert_eq!(table.get(0x1111u64), Some("alpha"));
}

/// An offset past the data section is rejected at read time instead of
/// panicking on lookup.
#[test]
fn read_rejects_out_of_bounds_offset() {
    let hash_bits = 38u8;
    let bad_offset = 9999u64;
    let entry = (0x1234u64 & ((1u64 << hash_bits) - 1)) | (bad_offset << hash_bits);

    let mut out = Vec::new();
    out.extend_from_slice(b"RST");
    out.push(5);
    out.extend_from_slice(&1u32.to_le_bytes()); // count = 1
    out.extend_from_slice(&entry.to_le_bytes());
    out.extend_from_slice(b"hi\0"); // tiny data section, far smaller than bad_offset

    let err = Stringtable::from_reader(&mut Cursor::new(out)).unwrap_err();
    assert!(matches!(err, RstError::InvalidOffset { .. }), "got {err:?}");
}

/// Writing is deterministic and the entry table is ordered by hash.
#[test]
fn write_is_deterministic_and_sorted() {
    let mut table = Stringtable::new();
    for i in 0..50u64 {
        table.insert(i.wrapping_mul(0x9E37_79B9_7F4A_7C15), format!("value {i}"));
    }

    let mut a = Vec::new();
    let mut b = Vec::new();
    table.to_writer(&mut a).unwrap();
    table.to_writer(&mut b).unwrap();
    assert_eq!(a, b, "writing the same table twice must be byte-identical");

    // V5 layout: magic(3) + version(1) + count(4), then count * u64 entries.
    let count = u32::from_le_bytes(a[4..8].try_into().unwrap()) as usize;
    let mask = table.format().hash_mask();
    let mut prev = 0u64;
    for i in 0..count {
        let start = 8 + i * 8;
        let raw = u64::from_le_bytes(a[start..start + 8].try_into().unwrap());
        let hash = raw & mask;
        assert!(hash >= prev, "entry table is not sorted by hash");
        prev = hash;
    }
}

/// An encrypted (`0xFF`-marked) entry is detected and round-trips under V5, which
/// carries no encryption-flag byte.
#[test]
fn v5_encrypted_entry_round_trips() {
    let hash_bits = 38u8;
    let mask = (1u64 << hash_bits) - 1;

    let mut blob = Vec::new();
    let normal_off = blob.len() as u64;
    blob.extend_from_slice(b"hello\0");
    let enc_off = blob.len() as u64;
    let payload = [0xDEu8, 0xAD, 0xBE, 0xEF];
    blob.push(0xFF);
    blob.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    blob.extend_from_slice(&payload);

    let h_normal = 0x111u64 & mask;
    let h_enc = 0x222u64 & mask;
    let packed = [
        h_normal | (normal_off << hash_bits),
        h_enc | (enc_off << hash_bits),
    ];

    let mut out = Vec::new();
    out.extend_from_slice(b"RST");
    out.push(5); // V5: no font config, no encryption-flag byte
    out.extend_from_slice(&(packed.len() as u32).to_le_bytes());
    for e in packed {
        out.extend_from_slice(&e.to_le_bytes());
    }
    out.extend_from_slice(&blob);

    let table = Stringtable::from_reader(&mut Cursor::new(&out)).expect("parse v5");
    assert_eq!(table.get(h_normal), Some("hello"));
    assert!(table.is_encrypted(h_enc), "0xFF entry should be encrypted");
    assert_eq!(table.get_encrypted(h_enc), Some(&payload[..]));

    let mut buf = Vec::new();
    table.to_writer(&mut buf).expect("serialise");
    let reloaded = Stringtable::from_reader(&mut Cursor::new(&buf)).expect("re-parse");
    assert_eq!(reloaded.get(h_normal), Some("hello"));
    assert!(
        reloaded.is_encrypted(h_enc),
        "encrypted entry lost on V5 round-trip"
    );
    assert_eq!(reloaded.get_encrypted(h_enc), Some(&payload[..]));
}

/// `to_latest` upgrades an XXH3 table losslessly but refuses an xxHash64 one.
#[test]
fn to_latest_upgrades_xxh3_rejects_xxh64() {
    // 39-bit XXH3 (patch 14.15–15.1) -> 38-bit V5: lossless, lookup preserved.
    let mut t = Stringtable::with_format(RstFormat::new(
        RstVersion::V5,
        HashBits::B39,
        RstHashAlgo::Xxh3,
    ));
    t.insert_str("game_client_quit", "Quit");
    t.to_latest().expect("xxh3 table should upgrade");
    assert_eq!(t.format(), RstFormat::LATEST);
    assert_eq!(t.get_key("game_client_quit"), Some("Quit"));

    // xxHash64 table can't be re-hashed to XXH3 without the original keys.
    let mut legacy = Stringtable::with_format(RstFormat::new(
        RstVersion::V3,
        HashBits::B40,
        RstHashAlgo::Xxh64,
    ));
    legacy.insert_str("foo", "bar");
    let err = legacy.to_latest().unwrap_err();
    assert!(
        matches!(err, RstError::IncompatibleAlgo { .. }),
        "got {err:?}"
    );
}

/// `set_format` refuses to widen the bit-width — the masked-off bits are gone.
#[test]
fn set_format_rejects_widening() {
    let mut table = Stringtable::new(); // V5 / 38-bit / XXH3
    table.insert_str("foo", "bar");
    let err = table
        .set_format(RstFormat::new(
            RstVersion::V5,
            HashBits::B39,
            RstHashAlgo::Xxh3,
        ))
        .unwrap_err();
    assert!(matches!(err, RstError::CannotWiden { .. }), "got {err:?}");
}

/// Narrowing collapses entries that alias to the same masked hash, and the
/// survivor is deterministic: the value of the numerically largest original
/// hash wins.
#[test]
fn set_format_narrowing_collapse_is_deterministic() {
    let low = 0x1234u64;
    let high = low | (1 << 38); // differs only in the bit that narrowing drops
    for _ in 0..8 {
        let mut table = Stringtable::with_format(RstFormat::new(
            RstVersion::V5,
            HashBits::B39,
            RstHashAlgo::Xxh3,
        ));
        table.insert(low, "low");
        table.insert(high, "high");
        table
            .set_format(RstFormat::new(
                RstVersion::V5,
                HashBits::B38,
                RstHashAlgo::Xxh3,
            ))
            .unwrap();
        assert_eq!(table.len(), 1, "aliasing entries must collapse");
        assert_eq!(table.get(low), Some("high"), "largest original hash wins");
    }
}

/// Equality is semantic: a table compares equal to its written-and-reloaded
/// self even though values move from owned slots into the data blob.
#[test]
fn eq_is_semantic_not_representational() {
    let mut table = Stringtable::new();
    table.insert_str("game_client_quit", "Quit");
    table.insert_str("game_client_play", "Play");

    let mut buf = Vec::new();
    table.to_writer(&mut buf).unwrap();
    let reloaded = Stringtable::from_reader(&mut Cursor::new(buf)).unwrap();
    assert_eq!(table, reloaded);

    let mut edited = reloaded.clone();
    edited.insert_str("game_client_quit", "Exit");
    assert_ne!(table, edited);
}

/// Lookups and inserts mask a full (untruncated) 64-bit hash down to the
/// table's bit-width, so callers holding a raw xxhash don't silently miss.
#[test]
fn full_width_hashes_are_masked_on_lookup_and_insert() {
    let mut table = Stringtable::new(); // V5 / 38-bit
    let full = 0xDEAD_BEEF_CAFE_F00Du64;
    let masked = full & ((1u64 << 38) - 1);

    table.insert(full, "value");
    assert_eq!(table.len(), 1);
    assert_eq!(table.get(masked), Some("value"));
    assert_eq!(table.get(full), Some("value"));
    assert!(table.contains(full));
    assert!(table.remove(full));
    assert!(table.is_empty());
}

/// Inserting, editing, extending, then writing and re-reading preserves data.
#[test]
fn edit_extend_and_write_round_trip() {
    let mut table = Stringtable::new(); // latest: V5 / 38-bit / XXH3
    table.insert_str("game_client_quit", "Quit");
    table.insert_str("game_client_play", "Play");

    // edit an existing entry
    table.insert_str("game_client_quit", "Exit");
    // extend with another
    table.insert_str("game_client_settings", "Settings");
    // remove one
    assert!(table.remove(table.format().hash_of("game_client_play")));

    assert_eq!(table.len(), 2);
    assert_eq!(table.get_key("game_client_quit"), Some("Exit"));
    assert_eq!(table.get_key("game_client_play"), None);

    let mut buf = Vec::new();
    table.to_writer(&mut buf).expect("serialise failed");

    let reloaded = Stringtable::from_reader(&mut Cursor::new(buf)).expect("re-parse failed");
    assert_eq!(reloaded.len(), 2);
    assert_eq!(reloaded.get_key("game_client_quit"), Some("Exit"));
    assert_eq!(reloaded.get_key("game_client_settings"), Some("Settings"));
}

/// Identical string values are stored once in the serialised output.
#[test]
fn writer_deduplicates_strings() {
    let mut table = Stringtable::new();
    let shared = "Shared string value";
    for i in 0u64..10 {
        table.insert(i, shared);
    }

    let mut buf = Vec::new();
    table.to_writer(&mut buf).expect("serialise failed");

    let occurrences = buf
        .windows(shared.len())
        .filter(|w| *w == shared.as_bytes())
        .count();
    assert_eq!(
        occurrences, 1,
        "string should appear exactly once in output"
    );
}

/// `for_patch` reproduces the documented algorithm/bit-width rules.
#[test]
fn for_patch_matches_known_rules() {
    // pre-14.15: xxHash64
    assert_eq!(
        RstFormat::for_patch(RstVersion::V5, 1400).hash_algo,
        RstHashAlgo::Xxh64
    );
    // 14.15+: XXH3
    assert_eq!(
        RstFormat::for_patch(RstVersion::V5, 1415).hash_algo,
        RstHashAlgo::Xxh3
    );
    // v4/v5 bit-width flips 39 -> 38 at patch 15.2
    assert_eq!(
        RstFormat::for_patch(RstVersion::V5, 1501).hash_bits,
        HashBits::B39
    );
    assert_eq!(
        RstFormat::for_patch(RstVersion::V5, 1502).hash_bits,
        HashBits::B38
    );
    // v2/v3 always 40
    assert_eq!(
        RstFormat::for_patch(RstVersion::V2, 1600).hash_bits,
        HashBits::B40
    );
}

/// Hashing a key is case-insensitive (keys are lowercased before hashing).
#[test]
fn hash_of_is_case_insensitive() {
    let format = RstFormat::LATEST;
    let lower = format.hash_of("game_client_quit");
    let upper = format.hash_of("GAME_CLIENT_QUIT");
    let mixed = format.hash_of("Game_Client_Quit");
    assert_eq!(lower, upper);
    assert_eq!(lower, mixed);
}

/// A hash is masked to its format's bit-width.
#[test]
fn hash_of_respects_bit_width() {
    let simple = RstFormat::new(RstVersion::V5, HashBits::B38, RstHashAlgo::Xxh3);
    let complex = RstFormat::new(RstVersion::V2, HashBits::B40, RstHashAlgo::Xxh64);
    assert!(*simple.hash_of("some_key") < (1u64 << 38));
    assert!(*complex.hash_of("some_key") < (1u64 << 40));
}

/// Keys are ASCII-lowercased only (real keys are always ASCII); non-ASCII
/// bytes are hashed verbatim and do not fold.
#[test]
fn hash_of_lowercases_ascii_only() {
    let format = RstFormat::LATEST;
    assert_eq!(format.hash_of("CAFé"), format.hash_of("café"));
    assert_ne!(format.hash_of("CAFÉ"), format.hash_of("café"));
}

/// `set_hash_algo` changes which algorithm `hash_of`/`get_key` use, so a key
/// inserted under one algorithm is no longer found under another.
#[test]
fn set_hash_algo_changes_lookup() {
    let mut table = Stringtable::new(); // XXH3
    assert_eq!(table.format().hash_algo, RstHashAlgo::Xxh3);
    table.insert_str("game_client_quit", "Quit");
    assert_eq!(table.get_key("game_client_quit"), Some("Quit"));

    // Switching the algorithm makes the same key hash differently -> miss.
    table.set_hash_algo(RstHashAlgo::Xxh64);
    assert_eq!(table.format().hash_algo, RstHashAlgo::Xxh64);
    assert_eq!(table.get_key("game_client_quit"), None);

    // Switching back restores the lookup.
    table.set_hash_algo(RstHashAlgo::Xxh3);
    assert_eq!(table.get_key("game_client_quit"), Some("Quit"));
}

/// `pack` masks the hash to `hash_bits`, so a hash carried over from a wider
/// format can't corrupt the offset field.
#[test]
fn pack_masks_oversized_hash() {
    let format = RstFormat::new(RstVersion::V5, HashBits::B38, RstHashAlgo::Xxh3);
    // A hash with every bit set (as if carried over from a 40-bit table).
    let raw = format.pack(RstHash::from(u64::MAX), 5);
    let (hash, offset) = format.unpack(raw);
    assert_eq!(offset, 5, "offset must survive packing a wider hash");
    assert_eq!(*hash, (1u64 << 38) - 1, "hash is truncated to hash_bits");
}

#[test]
fn invalid_magic_returns_error() {
    let bad = b"\x00\x00\x00\x05";
    let err = Stringtable::from_reader(&mut Cursor::new(bad)).unwrap_err();
    assert!(
        matches!(err, RstError::InvalidMagic { .. }),
        "expected InvalidMagic, got {err:?}"
    );
}

#[test]
fn unsupported_version_returns_error() {
    let bad = b"RST\x01";
    let err = Stringtable::from_reader(&mut Cursor::new(bad)).unwrap_err();
    assert!(
        matches!(err, RstError::UnsupportedVersion { version: 0x01 }),
        "expected UnsupportedVersion(0x01), got {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Real-file tests (committed fixtures, always run)
// ---------------------------------------------------------------------------

/// An owned, comparable snapshot of every entry (covers strings, encrypted, and
/// invalid-UTF-8 entries) for lossless round-trip checks.
fn snapshot(table: &Stringtable) -> std::collections::HashMap<u64, (u8, Vec<u8>)> {
    table
        .keys()
        .map(|h| {
            let kv = if let Some(b) = table.get_encrypted(h) {
                (1u8, b.to_vec())
            } else if let Some(s) = table.get(h) {
                (0u8, s.as_bytes().to_vec())
            } else {
                // Non-UTF-8 entry: none in the committed fixtures, but cover the
                // kind so the snapshot stays total.
                (
                    2u8,
                    table
                        .get_lossy(h)
                        .unwrap_or_default()
                        .into_owned()
                        .into_bytes(),
                )
            };
            (*h, kv)
        })
        .collect()
}

/// Each real V5 `bootstrap` fixture parses, and the hash/offset split is
/// auto-detected correctly: 39-bit before patch 15.2, 38-bit from 15.2 on.
#[test]
fn real_files_detect_correct_hash_bits() {
    for (name, expected_bits, expected_len) in [
        ("bootstrap.stringtable.1501", HashBits::B39, 143usize), // patch 15.1
        ("bootstrap.stringtable.latest", HashBits::B38, 201),    // patch >= 15.2
    ] {
        let bytes = read_fixture(name);
        let table = Stringtable::reader()
            .detect_hash_bits()
            .read(&mut Cursor::new(&bytes))
            .unwrap_or_else(|e| panic!("{name}: failed to parse: {e}"));

        assert_eq!(table.format().version, RstVersion::V5, "{name}: version");
        assert_eq!(table.format().hash_bits, expected_bits, "{name}: hash_bits");
        assert_eq!(table.len(), expected_len, "{name}: entry count");
        // Every entry must resolve to a valid UTF-8 string (no garbage offsets).
        assert_eq!(
            table.iter().count(),
            expected_len,
            "{name}: some entries did not resolve to valid strings"
        );
    }
}

/// A stable known hash resolves to the same string across the XXH3-era files.
#[test]
fn real_files_known_entry() {
    for name in ["bootstrap.stringtable.1501", "bootstrap.stringtable.latest"] {
        let bytes = read_fixture(name);
        let table = Stringtable::reader()
            .detect_hash_bits()
            .read(&mut Cursor::new(&bytes))
            .unwrap();
        assert_eq!(table.get(0x8818cc3cu64), Some("Retry"), "{name}");
    }
}

/// The patch-10.9 V2 fontconfig file exercises V2 framing: the font-config
/// block, 40-bit hashing, and encrypted (`trenc`) entries.
#[test]
fn fontconfig_v2_parses() {
    let bytes = read_fixture("fontconfig_en_us.txt.1009");
    let table = Stringtable::from_reader(&mut Cursor::new(&bytes)).expect("failed to parse");

    assert_eq!(table.format().version, RstVersion::V2);
    assert_eq!(table.format().hash_bits, HashBits::B40);
    assert_eq!(table.format().hash_algo, RstHashAlgo::Xxh64);
    assert_eq!(table.len(), 47017);

    assert!(table
        .font_config_str()
        .is_some_and(|c| c.starts_with("[FontConfig \"English\"]")));

    // Three entries are encrypted (0xFF-marked) rather than NUL-terminated.
    let encrypted = table.keys().filter(|&h| table.is_encrypted(h)).count();
    assert_eq!(encrypted, 3, "expected 3 encrypted entries");
}

/// The patch-11.3 V3 fontconfig file exercises V3 framing: 40-bit hashing, a
/// encryption-flag byte (V3 has no font-config block), and encrypted entries.
#[test]
fn fontconfig_v3_parses() {
    let bytes = read_fixture("fontconfig_en_us.txt.1103");
    let table = Stringtable::from_reader(&mut Cursor::new(&bytes)).expect("failed to parse");

    assert_eq!(table.format().version, RstVersion::V3);
    assert_eq!(table.format().hash_bits, HashBits::B40);
    assert_eq!(table.format().hash_algo, RstHashAlgo::Xxh64);
    assert_eq!(table.len(), 58156);
    assert_eq!(table.font_config(), None, "V3 has no font-config block");

    let encrypted = table.keys().filter(|&h| table.is_encrypted(h)).count();
    assert_eq!(encrypted, 3, "expected 3 encrypted entries");
}

/// Reading, writing, and re-reading each real file preserves the detected
/// format, the font config, and every entry (including encrypted ones).
#[test]
fn real_files_round_trip() {
    for name in [
        "bootstrap.stringtable.1501",
        "bootstrap.stringtable.latest",
        "fontconfig_en_us.txt.1009",
        "fontconfig_en_us.txt.1103",
    ] {
        let bytes = read_fixture(name);
        let original = Stringtable::reader()
            .detect_hash_bits()
            .read(&mut Cursor::new(&bytes))
            .unwrap();

        let mut buf = Vec::new();
        original.to_writer(&mut buf).expect("serialise failed");
        let reloaded = Stringtable::reader()
            .detect_hash_bits()
            .read(&mut Cursor::new(&buf))
            .expect("re-parse failed");

        assert_eq!(
            reloaded.format().hash_bits,
            original.format().hash_bits,
            "{name}: detected hash_bits changed after round-trip"
        );
        assert_eq!(
            reloaded.font_config(),
            original.font_config(),
            "{name}: font config changed after round-trip"
        );
        assert_eq!(
            snapshot(&reloaded),
            snapshot(&original),
            "{name}: entries changed after round-trip"
        );
        assert_eq!(
            reloaded, original,
            "{name}: tables should compare equal after round-trip"
        );
    }
}
