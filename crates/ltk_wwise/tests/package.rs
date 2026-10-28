//! Reading, editing, writing and mounting WPK packages.

mod common;

use std::io::{self, Cursor};

use ltk_wwise::{
    package::{MountedEntry, PackageEntry, PackageKey},
    Error, MountedPackage, Package, SourceId,
};

fn write(package: &Package) -> Vec<u8> {
    let mut bytes = Vec::new();
    package.to_writer(&mut bytes).unwrap();
    bytes
}

fn utf16(name: &str) -> Vec<u8> {
    name.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// Returns the package of `1.wem` (5 bytes) and `22.wem` (9 bytes).
fn two_wems() -> Package {
    let mut package = Package::new();
    assert_eq!(package.insert_wem(SourceId(1), b"RIFF1".to_vec()), None);
    assert_eq!(
        package.insert_wem(SourceId(22), b"RIFF22222".to_vec()),
        None
    );
    package
}

#[test]
fn fixture_writes_back_byte_for_byte() {
    let bytes = common::fixture("package.wpk");

    let package = Package::from_reader(&mut bytes.as_slice()).unwrap();

    let names: Vec<_> = package.entries().iter().map(PackageEntry::name).collect();
    assert_eq!(names, ["380333190.wem", "470358536.wem"]);
    assert_eq!(write(&package), bytes);
}

#[test]
fn built_package_writes_the_aligned_layout() {
    let package = two_wems();

    // Header and table end at 20. Record 0 spans 24..46, record 1 spans 48..72.
    // Media 0 spans 72..77, media 1 spans 80..89.
    let mut expected = Vec::new();
    expected.extend_from_slice(b"r3d2");
    for value in [1u32, 2, 24, 48] {
        expected.extend_from_slice(&value.to_le_bytes());
    }
    expected.extend_from_slice(&[0; 4]);
    for value in [72u32, 5, 5] {
        expected.extend_from_slice(&value.to_le_bytes());
    }
    expected.extend_from_slice(&utf16("1.wem"));
    expected.extend_from_slice(&[0; 2]);
    for value in [80u32, 9, 6] {
        expected.extend_from_slice(&value.to_le_bytes());
    }
    expected.extend_from_slice(&utf16("22.wem"));
    expected.extend_from_slice(b"RIFF1");
    expected.extend_from_slice(&[0; 3]);
    expected.extend_from_slice(b"RIFF22222");

    let bytes = write(&package);
    assert_eq!(bytes, expected);

    let read = Package::from_reader(&mut bytes.as_slice()).unwrap();
    assert_eq!(read, package);
    assert_eq!(read.len(), 2);
    assert_eq!(read.wem(SourceId(22)), Some(&b"RIFF22222"[..]));
}

#[test]
fn empty_package_writes_the_header_only() {
    let bytes = write(&Package::new());

    assert_eq!(bytes, b"r3d2\x01\0\0\0\0\0\0\0");
    assert!(Package::from_reader(&mut bytes.as_slice())
        .unwrap()
        .is_empty());
}

#[test]
fn insert_wem_of_an_existing_source_replaces_the_media_in_place() {
    let mut package = two_wems();

    let old = package.insert_wem(SourceId(1), b"RIFFnew".to_vec());

    assert_eq!(old, Some(b"RIFF1".to_vec()));
    let names: Vec<_> = package.entries().iter().map(PackageEntry::name).collect();
    assert_eq!(names, ["1.wem", "22.wem"]);
    assert_eq!(package.wem(SourceId(1)), Some(&b"RIFFnew"[..]));
}

#[test]
fn insert_of_a_new_key_appends_and_remove_keeps_the_order() {
    let mut package = two_wems();

    assert_eq!(
        package.insert(PackageEntry::new("Music.wem", b"RIFFm".to_vec())),
        None
    );
    let removed = package.remove(PackageKey(1)).unwrap();

    assert_eq!(removed.name(), "1.wem");
    let names: Vec<_> = package.entries().iter().map(PackageEntry::name).collect();
    assert_eq!(names, ["22.wem", "Music.wem"]);
    assert_eq!(
        package
            .get(PackageKey::from_name("music.wem"))
            .map(PackageEntry::data),
        Some(&b"RIFFm"[..])
    );
    assert_eq!(package.remove(PackageKey(1)), None);
}

#[test]
fn package_key_is_the_leading_digits_of_the_name() {
    assert_eq!(
        PackageKey::from_name("1004362275.wem"),
        PackageKey(1004362275)
    );
    assert_eq!(PackageKey::from_name("0042.wem"), PackageKey(42));
    assert_eq!(
        PackageEntry::wem(SourceId(7), Vec::new()).key(),
        PackageKey(7)
    );
}

#[test]
fn package_key_without_leading_digits_is_the_elf_hash_of_the_lowercased_name() {
    // Reference values from an independent ELF hash implementation.
    assert_eq!(
        PackageKey::from_name("JDFGSDHFSDFSD 6445dsfsd7fg/*/+bfjsdgf%$^"),
        PackageKey(248446350)
    );
    assert_eq!(PackageKey::from_name("Music.wem"), PackageKey(167127421));
    assert_eq!(PackageKey::from_name("0.wem"), PackageKey(3366333));
    assert_eq!(
        PackageEntry::wem(SourceId(0), Vec::new()).key(),
        PackageKey(3366333)
    );
    assert_eq!(
        PackageKey::from_name("99999999999999999999.wem"),
        PackageKey(252219773)
    );
}

#[test]
fn reader_rejects_a_wrong_magic() {
    let mut bytes = write(&two_wems());
    bytes[..4].copy_from_slice(b"r3d3");

    let error = Package::from_reader(&mut bytes.as_slice()).unwrap_err();

    assert!(matches!(
        error,
        Error::InvalidMagic { expected, found } if &expected == b"r3d2" && &found == b"r3d3"
    ));
}

#[test]
fn reader_rejects_a_version_other_than_1() {
    let mut bytes = write(&two_wems());
    bytes[4..8].copy_from_slice(&2u32.to_le_bytes());

    let error = Package::from_reader(&mut bytes.as_slice()).unwrap_err();

    assert!(matches!(error, Error::PackageVersion(2)));
}

#[test]
fn reader_rejects_a_duplicate_key() {
    let mut package = Package::new();
    package.insert_wem(SourceId(1), b"RIFF1".to_vec());
    package.insert_wem(SourceId(2), b"RIFF2".to_vec());
    let mut bytes = write(&package);
    let second = utf16("2.wem");
    let at = bytes
        .windows(second.len())
        .position(|window| window == second)
        .unwrap();
    bytes[at] = b'1';

    let error = Package::from_reader(&mut bytes.as_slice()).unwrap_err();

    assert!(matches!(error, Error::DuplicateKey(PackageKey(1))));
}

#[test]
fn reader_rejects_a_media_that_ends_after_the_end_of_the_file() {
    let mut bytes = write(&two_wems());
    bytes.pop();

    let error = Package::from_reader(&mut bytes.as_slice()).unwrap_err();

    assert!(matches!(error, Error::EntryOutOfBounds { name } if name == "22.wem"));
}

#[test]
fn reader_rejects_a_record_that_ends_after_the_end_of_the_file() {
    let mut bytes = write(&two_wems());
    bytes[16..20].copy_from_slice(&80u32.to_le_bytes());

    let error = Package::from_reader(&mut bytes.as_slice()).unwrap_err();

    assert!(matches!(error, Error::EntryOutOfBounds { name } if name == "#1"));
}

#[test]
fn reader_rejects_an_offset_table_that_ends_after_the_end_of_the_file() {
    let mut bytes = write(&two_wems());
    bytes[8..12].copy_from_slice(&u32::MAX.to_le_bytes());

    let error = Package::from_reader(&mut bytes.as_slice()).unwrap_err();

    assert!(matches!(error, Error::Io(e) if e.kind() == io::ErrorKind::UnexpectedEof));
}

/// A reader that fails every read.
struct Failing;

impl io::Read for Failing {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("read past the end of the package"))
    }
}

#[test]
fn reader_reads_no_further_than_the_end_of_the_last_media() {
    let bytes = common::fixture("package.wpk");

    let package = Package::from_reader(&mut io::Read::chain(bytes.as_slice(), Failing)).unwrap();

    assert_eq!(write(&package), bytes);
}

/// Returns the bytes of [`two_wems`] with the data offset and size of each record replaced.
fn two_wems_with_media(spans: [(u32, u32); 2]) -> Vec<u8> {
    let mut bytes = write(&two_wems());
    for (record, (offset, size)) in [24, 48].into_iter().zip(spans) {
        bytes[record..record + 4].copy_from_slice(&offset.to_le_bytes());
        bytes[record + 4..record + 8].copy_from_slice(&size.to_le_bytes());
    }
    bytes
}

#[test]
fn reader_reads_media_in_any_order_and_media_that_entries_share() {
    // The media of `22.wem` at 72..77 before the media of `1.wem` at 80..89.
    let swapped = two_wems_with_media([(80, 9), (72, 5)]);
    let package = Package::from_reader(&mut swapped.as_slice()).unwrap();
    assert_eq!(package.wem(SourceId(1)), Some(&b"RIFF22222"[..]));
    assert_eq!(package.wem(SourceId(22)), Some(&b"RIFF1"[..]));

    // Both entries name the media at 72..77.
    let shared = two_wems_with_media([(72, 5), (72, 5)]);
    let package = Package::from_reader(&mut shared.as_slice()).unwrap();
    assert_eq!(package.wem(SourceId(1)), Some(&b"RIFF1"[..]));
    assert_eq!(package.wem(SourceId(22)), Some(&b"RIFF1"[..]));

    // The media of `1.wem` names the bytes of the offset table.
    let in_table = two_wems_with_media([(12, 8), (80, 9)]);
    let package = Package::from_reader(&mut in_table.as_slice()).unwrap();
    assert_eq!(package.wem(SourceId(1)), Some(&in_table[12..20]));
}

#[test]
fn mounted_fixture_lists_the_entries_and_reads_the_media_of_the_read_package() {
    let bytes = common::fixture("package.wpk");
    let package = Package::from_reader(&mut bytes.as_slice()).unwrap();

    let mut mounted = MountedPackage::mount(Cursor::new(bytes.as_slice())).unwrap();

    let entries = mounted.entries().to_vec();
    assert_eq!(entries.len(), package.len());
    for (record, entry) in entries.iter().zip(package.entries()) {
        assert_eq!(record.name(), entry.name());
        assert_eq!(record.key(), entry.key());
        assert_eq!(record.size() as usize, entry.data().len());
        assert_eq!(mounted.read(record).unwrap(), entry.data());
    }
    assert_eq!(entries[0].offset(), 0x68);
    assert_eq!(
        mounted.get(PackageKey(470358536)).map(MountedEntry::name),
        Some("470358536.wem")
    );
    assert_eq!(mounted.to_package().unwrap(), package);
    assert_eq!(mounted.into_inner().into_inner(), bytes.as_slice());
}

#[test]
fn mount_applies_the_checks_of_the_reader() {
    let mut bytes = write(&two_wems());
    bytes.pop();

    let error = MountedPackage::mount(Cursor::new(bytes)).unwrap_err();

    assert!(matches!(error, Error::EntryOutOfBounds { name } if name == "22.wem"));
}
