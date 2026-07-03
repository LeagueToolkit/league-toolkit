//! Integration tests mounting real WAD samples.

use std::io::Cursor;

use ltk_wad::rsa::pkcs8::DecodePublicKey as _;
use ltk_wad::rsa::RsaPublicKey;
use ltk_wad::{Wad, WadBuilder, RITO_PKEY};

/// Header-only v3.4 wad with an empty TOC.
const EMPTY_WAD: &[u8] = include_bytes!("wads/Online.wad.client");
/// Small v3.4 wad with 6 chunks.
const SMALL_WAD: &[u8] = include_bytes!("wads/UI.en_US.wad.client");

/// SHA-256 of zero bytes.
const EMPTY_TOC_SHA256: [u8; 32] = [
    0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9, 0x24,
    0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52, 0xb8, 0x55,
];

/// SHA-256 of `SMALL_WAD[272..464]`, its on-disk TOC.
const SMALL_TOC_SHA256: [u8; 32] = [
    0x1e, 0xe1, 0x92, 0x63, 0xef, 0xb2, 0xe2, 0x43, 0xc8, 0xd2, 0x89, 0xd5, 0x58, 0x35, 0x7d, 0x0b,
    0x03, 0xcb, 0x15, 0xde, 0x2c, 0xda, 0x28, 0xe3, 0x98, 0x2f, 0x6f, 0xcb, 0x9a, 0x50, 0x65, 0x06,
];

#[test]
fn mount_empty_wad() {
    let wad = Wad::mount(Cursor::new(EMPTY_WAD)).expect("Failed to mount WAD");

    assert!(wad.chunks().is_empty());
    assert_ne!(wad.signature(), &[0u8; 256]);
    assert_ne!(wad.checksum(), 0);
    assert_eq!(wad.toc_sha256().unwrap(), EMPTY_TOC_SHA256);
}

#[test]
fn rebuild_empty_wad_byte_identical() {
    let wad = Wad::mount(Cursor::new(EMPTY_WAD)).expect("Failed to mount WAD");

    let mut cursor = Cursor::new(Vec::new());
    WadBuilder::default()
        .with_signature(wad.signature())
        .with_checksum(wad.checksum())
        .build_to_writer(&mut cursor, |_path, _cursor| Ok(()))
        .expect("Failed to build WAD");

    assert_eq!(cursor.get_ref().as_slice(), EMPTY_WAD);
}

#[test]
fn mount_small_wad() {
    let mut wad = Wad::mount(Cursor::new(SMALL_WAD)).expect("Failed to mount WAD");

    assert_eq!(wad.chunks().len(), 6);
    assert_ne!(wad.signature(), &[0u8; 256]);
    assert_eq!(wad.checksum(), 0x5FDFCE0422B0F537);
    assert_eq!(wad.toc_sha256().unwrap(), SMALL_TOC_SHA256);

    let chunks: Vec<_> = wad.chunks().iter().copied().collect();
    for chunk in &chunks {
        let raw = wad.load_chunk_raw(chunk).expect("Failed to read chunk");
        assert_eq!(raw.len(), chunk.compressed_size());
        assert_eq!(xxhash_rust::xxh3::xxh3_64(&raw), chunk.checksum());

        let decompressed = wad
            .load_chunk_decompressed(chunk)
            .expect("Failed to decompress chunk");
        assert_eq!(decompressed.len(), chunk.uncompressed_size());
    }
}

#[test]
fn verify_empty_wad_signature() {
    let key = RsaPublicKey::from_public_key_der(RITO_PKEY).expect("Failed to parse key");
    let wad = Wad::mount(Cursor::new(EMPTY_WAD)).expect("Failed to mount WAD");

    let (valid, toc_sha256) = wad.verify_signature(&key).unwrap();
    assert!(valid);
    assert_eq!(toc_sha256, EMPTY_TOC_SHA256);
}

#[test]
fn verify_small_wad_signature() {
    let key = RsaPublicKey::from_public_key_der(RITO_PKEY).expect("Failed to parse key");
    let wad = Wad::mount(Cursor::new(SMALL_WAD)).expect("Failed to mount WAD");

    let (valid, toc_sha256) = wad.verify_signature(&key).unwrap();
    assert!(valid);
    assert_eq!(toc_sha256, SMALL_TOC_SHA256);
}
