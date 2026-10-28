//! Banks through the public surface: read, edit, write, read again.

mod common;

use common::fixture;
use ltk_wwise::{
    bank::{BankHeader, BankType, BankVersion, RawSection, Rejection, Section},
    Bank, BankId, Error, MediaIndex, SourceId,
};

const BANK_FIXTURES: [&str; 5] = [
    "empty_v145.bnk",
    "media_v134.bnk",
    "media_v145.bnk",
    "events_v134.bnk",
    "events_v145.bnk",
];

fn read(bytes: &[u8]) -> Bank {
    Bank::from_reader(&mut &bytes[..]).expect("read bank")
}

fn write(bank: &Bank) -> Vec<u8> {
    let mut out = Vec::new();
    bank.to_writer(&mut out).expect("write bank");
    out
}

#[test]
fn every_bank_fixture_writes_back_byte_for_byte() {
    for name in BANK_FIXTURES {
        let bytes = fixture(name);
        assert_eq!(write(&read(&bytes)), bytes, "{name}");
    }
}

#[test]
fn header_decodes_the_fields_of_its_version() {
    let v145 = read(&fixture("events_v145.bnk"));
    let header = v145.header();
    assert_eq!(header.version, BankVersion::V145);
    assert_eq!(header.bank_id, BankId(2282428251));
    assert_eq!(header.language_id, 393239870);
    assert_eq!(header.alignment, 16);
    assert_eq!(header.project_id, 250);
    assert_eq!(header.bank_type, BankType::USER);
    assert_eq!(header.hash[..4], [0x22, 0xb5, 0x89, 0xca]);
    assert!(header.padding.is_empty());

    let v134 = read(&fixture("events_v134.bnk"));
    let header = v134.header();
    assert_eq!(header.version, BankVersion::V134);
    assert_eq!(header.alignment, 0);
    assert_eq!(header.hash, [0; 16]);
    assert_eq!(header.padding, [0; 4]);
}

#[test]
fn media_banks_expose_their_media_by_source_id() {
    for (name, source, len) in [
        ("media_v145.bnk", SourceId(879168912), 570),
        ("media_v134.bnk", SourceId(139840565), 428),
    ] {
        let bank = read(&fixture(name));
        assert_eq!(bank.header().media_alignment(), 16, "{name}");

        let media = bank.media().expect(name);
        assert_eq!(media.len(), 1, "{name}");
        assert!(media.contains(source), "{name}");
        let data = media.get(source).expect(name);
        assert_eq!(data.len(), len, "{name}");
        assert_eq!(&data[..4], b"RIFF", "{name}");
        assert_eq!(media.iter().map(|(id, _)| id).collect::<Vec<_>>(), [source]);
        assert_eq!(bank.sections().len(), 1, "{name}");
    }
}

#[test]
fn a_read_media_index_equals_a_built_one_with_the_same_media() {
    let bank = read(&fixture("media_v145.bnk"));
    let read_index = bank.media().unwrap();
    let mut built = MediaIndex::new();
    for (source, data) in read_index.iter() {
        built.insert(source, data.to_vec());
    }

    assert_eq!(*read_index, built);
    built.insert(SourceId(1), vec![0]);
    assert_ne!(*read_index, built);
}

/// Returns the body of the first chunk `tag` of the bank `bytes`.
fn chunk_body<'a>(bytes: &'a [u8], tag: &[u8; 4]) -> &'a [u8] {
    let mut rest = bytes;
    while rest.len() >= 8 {
        let size = u32::from_le_bytes(rest[4..8].try_into().unwrap()) as usize;
        let body = &rest[8..8 + size];
        if &rest[..4] == tag {
            return body;
        }
        rest = &rest[8 + size..];
    }
    panic!("no chunk {tag:?}");
}

/// Returns the `(source ID, offset, size)` records of the first `DIDX` of `bytes`.
fn didx_records(bytes: &[u8]) -> Vec<(u32, u32, u32)> {
    let field = |r: &[u8], at: usize| u32::from_le_bytes(r[at..at + 4].try_into().unwrap());
    chunk_body(bytes, b"DIDX")
        .as_chunks::<12>()
        .0
        .iter()
        .map(|r| (field(r, 0), field(r, 4), field(r, 8)))
        .collect()
}

#[test]
fn edited_media_is_laid_out_in_source_order_on_the_media_alignment() {
    let original = read(&fixture("media_v145.bnk"));
    let kept = SourceId(879168912);
    let kept_bytes = original.media().unwrap().get(kept).unwrap().to_vec();

    let mut bank = original.clone();
    let media = bank.media_mut().unwrap();
    assert_eq!(media.insert(SourceId(5), vec![1; 3]), None);
    assert_eq!(media.insert(SourceId(4_000_000_000), vec![2; 17]), None);
    assert_eq!(media.insert(SourceId(900_000_000), vec![3; 1]), None);
    assert_eq!(media.insert(SourceId(5), vec![4; 7]), Some(vec![1; 3]));
    assert_eq!(media.remove(SourceId(900_000_000)), Some(vec![3; 1]));
    assert_eq!(media.remove(SourceId(900_000_000)), None);

    let bytes = write(&bank);
    let records = didx_records(&bytes);
    assert_eq!(
        records,
        [(5, 0, 7), (879168912, 16, 570), (4_000_000_000, 592, 17)]
    );
    let data = chunk_body(&bytes, b"DATA");
    assert_eq!(data.len(), 592 + 17);
    assert_eq!(&data[7..16], [0; 9]);
    assert_eq!(&data[586..592], [0; 6]);

    let reread = read(&bytes);
    let media = reread.media().unwrap();
    assert_eq!(media.get(kept), Some(&kept_bytes[..]));
    assert_eq!(media.get(SourceId(5)), Some(&[4; 7][..]));
    assert_eq!(media.get(SourceId(4_000_000_000)), Some(&[2; 17][..]));
    assert!(!media.contains(SourceId(900_000_000)));
    assert_eq!(
        media.iter().map(|(id, _)| id.0).collect::<Vec<_>>(),
        [5, 879168912, 4_000_000_000]
    );
    let mut header = original.header().clone();
    header.padding = vec![0; 12];
    assert_eq!(*reread.header(), header);
}

#[test]
fn edited_media_of_a_legacy_bank_aligns_to_16() {
    let mut bank = read(&fixture("media_v134.bnk"));
    bank.media_mut()
        .unwrap()
        .insert(SourceId(u32::MAX), vec![9; 2]);

    let records = didx_records(&write(&bank));
    assert_eq!(records, [(139840565, 0, 428), (u32::MAX, 432, 2)]);
}

#[test]
fn an_unedited_media_index_lays_out_again_only_when_its_offsets_miss_the_alignment() {
    let mut bank = read(&media_bank(&[(1, 0, 3), (2, 16, 5)], 21));
    bank.header_mut().version = BankVersion::V145;

    bank.header_mut().alignment = 8;
    assert_eq!(didx_records(&write(&bank)), [(1, 0, 3), (2, 16, 5)]);

    bank.header_mut().alignment = 32;
    let bytes = write(&bank);
    assert_eq!(didx_records(&bytes), [(1, 0, 3), (2, 32, 5)]);
    assert_eq!(chunk_body(&bytes, b"DATA").len(), 37);
}

/// Returns the file offset of the first `DATA` body of the bank `bytes`.
fn data_offset(bytes: &[u8]) -> usize {
    chunk_body(bytes, b"DATA").as_ptr() as usize - bytes.as_ptr() as usize
}

#[test]
fn the_writer_pads_bkhd_with_zeros_to_start_data_on_the_media_alignment() {
    let mut media = MediaIndex::new();
    media.insert(SourceId(7), b"RIFF".to_vec());
    let mut bank = Bank::media_only("a.bnk", media);
    // BKHD 8 + 40, DIDX 8 + 12 and the DATA header 8 end at 76.
    assert_eq!(data_offset(&write(&bank)), 80);

    // A STID chunk of 8 + 5 bytes before the media ends the DATA header at 89.
    bank.sections_mut().insert(
        0,
        Section::Raw(RawSection {
            tag: *b"STID",
            data: vec![0; 5],
        }),
    );
    let bytes = write(&bank);
    assert_eq!(data_offset(&bytes), 96);
    assert_eq!(read(&bytes).header().padding, [0; 7]);

    // The fixture with three media: a DIDX of 36 bytes ends the DATA header at 100.
    let mut bank = read(&fixture("media_v145.bnk"));
    let media = bank.media_mut().unwrap();
    media.insert(SourceId(5), vec![1]);
    media.insert(SourceId(6), vec![2]);
    let bytes = write(&bank);
    assert_eq!(data_offset(&bytes), 112);
    assert_eq!(read(&bytes).header().padding, [0; 12]);
}

#[test]
fn the_writer_keeps_padding_that_starts_data_on_the_media_alignment() {
    // Without padding, the DATA header of the fixture ends at 76.
    let mut bank = read(&fixture("media_v145.bnk"));
    for kept in [4, 20] {
        bank.header_mut().padding = vec![0xAA; kept];
        assert_eq!(read(&write(&bank)).header().padding, vec![0xAA; kept]);
    }

    bank.header_mut().padding = vec![0xAA; 8];
    assert_eq!(read(&write(&bank)).header().padding, [0; 4]);
}

#[test]
fn a_bank_without_media_writes_its_padding_as_is() {
    let mut bank = read(&fixture("events_v145.bnk"));
    bank.header_mut().padding = vec![0xAA; 3];

    assert_eq!(read(&write(&bank)).header().padding, [0xAA; 3]);
}

#[test]
fn a_media_only_bank_is_a_current_user_bank_named_after_its_file() {
    let mut media = MediaIndex::new();
    media.insert(SourceId(7), b"RIFF".to_vec());
    let bank = Bank::media_only("sett_base_sfx_audio.bnk", media);
    assert!(bank.header().padding.is_empty());

    let reread = read(&write(&bank));
    let header = reread.header();
    assert_eq!(header.version, BankVersion(145));
    assert_eq!(header.bank_id, BankId(0xe9b70b40));
    assert_eq!(header.alignment, 16);
    assert_eq!(header.bank_type, BankType(0));
    assert_eq!(
        (header.language_id, header.alt_values, header.project_id),
        (0, 0, 0)
    );
    assert_eq!(header.hash, [0; 16]);
    // BKHD 8 + 40, DIDX 8 + 12 and the DATA header 8 end at 76: 4 bytes of padding give 80.
    assert_eq!(header.padding, [0; 4]);
    assert_eq!(reread.sections().len(), 1);
    assert_eq!(reread.media().unwrap().get(SourceId(7)), Some(&b"RIFF"[..]));
    assert_eq!(reread.check(), Ok(()));
}

#[test]
fn check_rejects_a_version_outside_118_to_145() {
    for version in [117, 146] {
        let bank = Bank::new(BankHeader::new(BankVersion(version), BankId(1)));
        assert_eq!(bank.check(), Err(Rejection::Version(BankVersion(version))));
    }
    for version in [118, 145] {
        let bank = Bank::new(BankHeader::new(BankVersion(version), BankId(1)));
        assert_eq!(bank.check(), Ok(()));
    }
}

#[test]
fn check_rejects_a_legacy_bank_with_a_chunk_other_than_didx_or_data() {
    let bank = read(&fixture("events_v134.bnk"));
    assert_eq!(
        bank.check(),
        Err(Rejection::LegacyChunk {
            version: BankVersion::V134,
            tag: *b"HIRC",
        })
    );
    assert_eq!(read(&fixture("media_v134.bnk")).check(), Ok(()));
    assert_eq!(read(&fixture("events_v145.bnk")).check(), Ok(()));
}

#[test]
fn check_rejects_a_bank_type_other_than_user_event_or_bus() {
    let mut bank = read(&fixture("media_v145.bnk"));
    for accepted in [BankType::USER, BankType::EVENT, BankType::BUS] {
        bank.header_mut().bank_type = accepted;
        assert_eq!(bank.check(), Ok(()));
    }
    bank.header_mut().bank_type = BankType(5);
    assert_eq!(bank.check(), Err(Rejection::BankType(BankType(5))));
}

#[test]
fn check_tests_the_version_before_the_bank_type() {
    let mut header = BankHeader::new(BankVersion(146), BankId(1));
    header.bank_type = BankType(5);
    assert_eq!(
        Bank::new(header).check(),
        Err(Rejection::Version(BankVersion(146)))
    );
}

fn read_err(bytes: &[u8]) -> Error {
    Bank::from_reader(&mut &bytes[..]).expect_err("bank must not read")
}

/// Returns a chunk of the tag `tag` and the body `body`.
fn chunk(tag: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = tag.to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
    out
}

/// Returns a version 134 bank of a `DIDX` of `records` and a `DATA` of `data_len` bytes.
///
/// The `BKHD` padding starts the `DATA` body at a multiple of 16, as in every shipped bank.
fn media_bank(records: &[(u32, u32, u32)], data_len: usize) -> Vec<u8> {
    let didx: Vec<u8> = records
        .iter()
        .flat_map(|&(id, offset, size)| [id, offset, size])
        .flat_map(u32::to_le_bytes)
        .collect();
    // The 20 bytes of the version 134 fields of the fixture, after the chunk header.
    let mut header = fixture("events_v134.bnk")[8..28].to_vec();
    let unpadded = 8 + header.len() + 8 + didx.len() + 8;
    header.resize(header.len() + unpadded.next_multiple_of(16) - unpadded, 0);
    let mut bytes = chunk(b"BKHD", &header);
    bytes.extend(chunk(b"DIDX", &didx));
    bytes.extend(chunk(b"DATA", &vec![0; data_len]));
    bytes
}

#[test]
fn the_reader_rejects_a_first_chunk_other_than_bkhd() {
    let bytes = fixture("events_v134.bnk");
    assert!(matches!(read_err(&bytes[32..]), Error::InvalidHeader));
    assert!(matches!(read_err(b""), Error::InvalidHeader));
    assert!(matches!(read_err(b"BKH"), Error::InvalidHeader));
}

#[test]
fn the_reader_rejects_a_header_shorter_than_its_version_requires() {
    let v134 = fixture("events_v134.bnk");
    assert!(matches!(
        read_err(&chunk(b"BKHD", &v134[8..27])),
        Error::InvalidHeader
    ));
    assert_eq!(read(&chunk(b"BKHD", &v134[8..28])).header().padding, []);

    let v145 = fixture("events_v145.bnk");
    assert!(matches!(
        read_err(&chunk(b"BKHD", &v145[8..47])),
        Error::InvalidHeader
    ));
}

#[test]
fn the_reader_rejects_a_chunk_that_ends_after_the_end_of_the_bank() {
    let bytes = fixture("events_v134.bnk");
    match read_err(&bytes[..bytes.len() - 1]) {
        Error::ChunkOverrun { tag, offset } => assert_eq!((tag, offset), (*b"HIRC", 32)),
        other => panic!("{other:?}"),
    }
    match read_err(&bytes[..36]) {
        Error::ChunkOverrun { tag, offset } => assert_eq!((tag, offset), (*b"HIRC", 32)),
        other => panic!("{other:?}"),
    }
    match read_err(&bytes[..20]) {
        Error::ChunkOverrun { tag, offset } => assert_eq!((tag, offset), (*b"BKHD", 0)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_reader_rejects_media_out_of_the_bounds_of_data() {
    assert!(read(&media_bank(&[(1, 0, 4), (2, 4, 4)], 8))
        .media()
        .is_some());
    for record in [(2, 5, 4), (2, 9, 0), (2, u32::MAX, 2)] {
        match read_err(&media_bank(&[(1, 0, 4), record], 8)) {
            Error::MediaOutOfBounds { source_id } => assert_eq!(source_id, SourceId(2)),
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn the_reader_rejects_two_records_of_one_source_id() {
    match read_err(&media_bank(&[(3, 0, 4), (1, 0, 4), (3, 4, 4)], 8)) {
        Error::DuplicateSource(source) => assert_eq!(source, SourceId(3)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_reader_accepts_records_in_any_order_and_writes_them_back_as_read() {
    let bytes = media_bank(&[(9, 16, 4), (1, 0, 4)], 20);
    let bank = read(&bytes);
    let ids: Vec<_> = bank.media().unwrap().iter().map(|(id, _)| id.0).collect();
    assert_eq!(ids, [1, 9]);
    assert_eq!(write(&bank), bytes);
}

#[test]
fn the_chunks_after_a_media_index_are_read_in_file_order() {
    let mut bytes = media_bank(&[(1, 0, 4)], 4);
    bytes.extend(chunk(b"HIRC", &[0; 4]));
    bytes.extend(chunk(b"STID", &[1; 3]));
    let bank = read(&bytes);
    let tags: Vec<_> = bank
        .sections()
        .iter()
        .map(|section| match section {
            Section::Media(_) => *b"DIDX",
            Section::Hierarchy(_) => *b"HIRC",
            Section::Raw(raw) => raw.tag,
        })
        .collect();
    assert_eq!(tags, [*b"DIDX", *b"HIRC", *b"STID"]);
    assert_eq!(write(&bank), bytes);
}

#[test]
fn a_didx_without_data_after_it_is_a_raw_section() {
    let mut bytes = fixture("events_v134.bnk")[..32].to_vec();
    bytes.extend(chunk(b"DIDX", &[]));
    bytes.extend(chunk(b"STID", &[]));
    bytes.extend(chunk(b"DATA", &[]));
    let bank = read(&bytes);
    assert!(bank.media().is_none());
    assert_eq!(bank.sections().len(), 3);
    assert_eq!(write(&bank), bytes);
}

#[test]
fn an_event_bank_has_no_media() {
    assert!(read(&fixture("events_v145.bnk")).media().is_none());
}
