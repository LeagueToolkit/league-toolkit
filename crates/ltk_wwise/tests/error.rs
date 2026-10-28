//! The messages of the errors of the crate.

use std::hash::Hash;

use ltk_wwise::{
    bank::{BankType, BankVersion, Rejection},
    hierarchy::ObjectKind,
    package::PackageKey,
    Error, ObjectId, SourceId, UpgradeRefusal,
};

#[test]
fn every_error_message_starts_lowercase_and_shows_values_with_display() {
    let cases = [
        (
            Error::InvalidMagic {
                expected: *b"r3d2",
                found: *b"BK\0\x01",
            },
            r"invalid magic: expected r3d2, found BK\x00\x01",
        ),
        (Error::PackageVersion(2), "unsupported package version 2"),
        (
            Error::DuplicateKey(PackageKey(5)),
            "two package entries have the key 5",
        ),
        (
            Error::EntryOutOfBounds {
                name: "5.wem".to_owned(),
            },
            "package entry 5.wem ends after the end of the package",
        ),
        (Error::InvalidHeader, "invalid bank header"),
        (
            Error::ChunkOverrun {
                tag: *b"DIDX",
                offset: 48,
            },
            "chunk DIDX at offset 48 ends after the end of the bank",
        ),
        (
            Error::MediaOutOfBounds {
                source_id: SourceId(7),
            },
            "media 7 ends after the end of DATA",
        ),
        (
            Error::DuplicateSource(SourceId(7)),
            "two media have the source ID 7",
        ),
        (
            Error::Framing { offset: 9, len: 8 },
            "hierarchy framing ends at 9, the chunk ends at 8",
        ),
        (
            Error::PayloadTooShort(3),
            "object payload of 3 bytes is shorter than an ID",
        ),
        (
            Error::UnsupportedLayout(BankVersion::V125),
            "no payload layouts for bank version 125",
        ),
        (
            Error::Layout {
                kind: ObjectKind::SOUND,
                id: ObjectId(5),
                offset: 10,
                len: 11,
            },
            "layout of Sound 5 stops at byte 10 of 11",
        ),
        (
            Error::ActionClass {
                id: ObjectId(5),
                class: 0x2a00,
            },
            "action 5 has the unknown class 0x2a00",
        ),
        (
            Error::Upgrade(UpgradeRefusal::Version(BankVersion::V125)),
            "upgrade refused: bank version 125 is not 132, 134 or 145",
        ),
        (Error::Wem("no RIFF header"), "invalid WEM: no RIFF header"),
    ];
    for (error, message) in cases {
        assert_eq!(error.to_string(), message);
    }
}

#[test]
fn upgrade_refusals_name_the_object_or_chunk_in_their_message() {
    let cases = [
        (
            UpgradeRefusal::Kind {
                id: ObjectId(5),
                kind: ObjectKind::MUSIC_SEGMENT,
            },
            "object 5 of kind MusicSegment has no upgrade transform",
        ),
        (
            UpgradeRefusal::ActionClass {
                id: ObjectId(5),
                class: 0x1500,
            },
            "action 5 of class 0x1500 has no version 145 counterpart",
        ),
    ];
    for (refusal, message) in cases {
        assert_eq!(refusal.to_string(), message);
    }
}

#[test]
fn rejections_show_the_tag_of_a_chunk_as_text() {
    let cases = [
        (
            Rejection::Version(BankVersion(146)),
            "bank version 146 is not 118 to 145",
        ),
        (
            Rejection::LegacyChunk {
                version: BankVersion::V134,
                tag: *b"HIRC",
            },
            "legacy bank version 134 has a HIRC chunk",
        ),
        (
            Rejection::BankType(BankType(2)),
            "bank type 2 is not 0, 30 or 31",
        ),
    ];
    for (rejection, message) in cases {
        assert_eq!(rejection.to_string(), message);
    }
}

fn is_hashable_error<E: std::error::Error + Hash + Send + Sync + 'static>() {}

#[test]
fn rejection_and_upgrade_refusal_are_hashable_errors() {
    is_hashable_error::<Rejection>();
    is_hashable_error::<UpgradeRefusal>();
}
