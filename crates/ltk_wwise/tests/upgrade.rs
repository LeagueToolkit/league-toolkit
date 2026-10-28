//! The upgrade of legacy banks to version 145 through the public surface.

mod common;

use common::fixture;
use ltk_wwise::{
    bank::{BankHeader, BankType, BankVersion},
    hierarchy::{Object, ObjectKind},
    Bank, BankId, Error, ObjectId, SourceId, UpgradeRefusal,
};

fn read(name: &str) -> Bank {
    Bank::from_reader(&mut &fixture(name)[..]).expect("read bank")
}

fn write(bank: &Bank) -> Vec<u8> {
    let mut out = Vec::new();
    bank.to_writer(&mut out).expect("write bank");
    out
}

/// Returns `bank` written and read again.
fn reread(bank: &Bank) -> Bank {
    Bank::from_reader(&mut &write(bank)[..]).expect("read upgraded bank")
}

#[test]
fn the_version_134_event_bank_upgrades_to_a_checked_version_145_bank() {
    let legacy = read("events_v134.bnk");

    let upgraded = reread(&legacy.upgrade().expect("upgrade"));

    let bytes = write(&upgraded);
    assert_eq!(&bytes[..4], b"BKHD");
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 40);
    let header = upgraded.header();
    assert_eq!(header.version, BankVersion::V145);
    assert_eq!(header.alignment, 16);
    assert_eq!(header.bank_type, BankType::USER);
    assert_eq!(header.hash, [0; 16]);
    assert!(header.padding.is_empty());
    assert_eq!(header.bank_id, legacy.header().bank_id);
    assert_eq!(header.language_id, legacy.header().language_id);
    assert_eq!(header.project_id, legacy.header().project_id);
    assert_eq!(upgraded.check(), Ok(()));

    let objects = upgraded.hierarchy().expect("hierarchy").objects();
    assert_eq!(objects.len(), legacy.hierarchy().unwrap().len());
    for object in objects {
        object
            .decode(BankVersion::V145)
            .unwrap_or_else(|e| panic!("{} {}: {e}", object.kind(), object.id()));
    }
}

/// The bytes of the 12 curve slots that version 145 adds to the 7 of version 134.
const NEW_CURVE_SLOTS: [u8; 12] = [0xFE; 12];

#[test]
fn each_object_grows_by_the_fields_that_version_145_adds() {
    let legacy = read("events_v134.bnk");
    let upgraded = legacy.upgrade().expect("upgrade");

    let pairs = legacy
        .hierarchy()
        .unwrap()
        .objects()
        .iter()
        .zip(upgraded.hierarchy().unwrap().objects());
    let mut growths = Vec::new();
    for (before, after) in pairs {
        assert_eq!((after.kind(), after.id()), (before.kind(), before.id()));
        let growth = after.payload().len() - before.payload().len();
        growths.push((before.kind().0, growth));
        if before.kind() == ObjectKind::ATTENUATION {
            // u32 id; the new u8; u8 cone; 20 bytes of cone when the cone is on; 7 old slots.
            let cone = if before.payload()[4] != 0 { 21 } else { 1 };
            let slots = 5 + cone + 7;
            assert_eq!(after.payload()[4], 0, "the new attenuation byte");
            assert_eq!(after.payload()[slots..slots + 12], NEW_CURVE_SLOTS);
        }
    }
    growths.sort_unstable();
    growths.dedup();
    // The fixture holds kinds 2, 3 (Play only), 4, 5, 7 and 14.
    assert_eq!(growths, [(2, 6), (3, 4), (4, 0), (5, 6), (7, 6), (14, 13)]);
}

#[test]
fn a_version_145_bank_upgrades_to_an_equal_bank() {
    for name in ["events_v145.bnk", "media_v145.bnk", "empty_v145.bnk"] {
        let bank = read(name);

        assert_eq!(bank.upgrade().expect(name), bank, "{name}");
    }
}

#[test]
fn the_version_134_media_bank_upgrades_with_its_media_unchanged_at_alignment_16() {
    let legacy = read("media_v134.bnk");
    assert_eq!(legacy.header().alignment, 0);

    let upgraded = reread(&legacy.upgrade().expect("upgrade"));

    assert_eq!(upgraded.header().version, BankVersion::V145);
    assert_eq!(upgraded.header().alignment, 16);
    assert_eq!(upgraded.check(), Ok(()));
    let media = |bank: &Bank| -> Vec<(SourceId, Vec<u8>)> {
        let index = bank.media().expect("media index");
        index.iter().map(|(id, data)| (id, data.to_vec())).collect()
    };
    assert_eq!(media(&upgraded), media(&legacy));

    // BKHD (8 + 40 bytes), then the DIDX records of source ID, offset and size.
    let bytes = write(&upgraded);
    assert_eq!(&bytes[48..52], b"DIDX");
    let didx_len = u32::from_le_bytes(bytes[52..56].try_into().unwrap()) as usize;
    for record in bytes[56..56 + didx_len].as_chunks::<12>().0 {
        let offset = u32::from_le_bytes(record[4..8].try_into().unwrap());
        assert_eq!(offset % 16, 0, "media offset {offset}");
    }
}

/// Returns the version 134 event bank with `object` appended to its hierarchy.
fn legacy_with(object: Object) -> Bank {
    let mut bank = read("events_v134.bnk");
    bank.hierarchy_mut().unwrap().push(object);
    bank
}

/// Returns the refusal of the upgrade of `bank`.
fn refusal(bank: &Bank) -> UpgradeRefusal {
    match bank.upgrade() {
        Err(Error::Upgrade(refusal)) => refusal,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_version_125_bank_is_refused() {
    let bank = Bank::new(BankHeader::new(BankVersion::V125, BankId(1)));

    assert_eq!(refusal(&bank), UpgradeRefusal::Version(BankVersion::V125));
}

#[test]
fn a_bank_with_a_music_segment_or_a_kind_17_object_is_refused() {
    for kind in [ObjectKind::MUSIC_SEGMENT, ObjectKind(17)] {
        let object = Object::new(kind, vec![0x78, 0x56, 0x34, 0x12, 0]).unwrap();

        assert_eq!(
            refusal(&legacy_with(object)),
            UpgradeRefusal::Kind {
                id: ObjectId(0x1234_5678),
                kind
            }
        );
    }
}

#[test]
fn a_bank_with_a_0x1500_action_is_refused() {
    // u32 id; u16 action type; u32 target; u8 target flags; two empty property bundles.
    let mut payload = 0x0BAD_F00Du32.to_le_bytes().to_vec();
    payload.extend_from_slice(&0x1500u16.to_le_bytes());
    payload.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0]);
    let action = Object::new(ObjectKind::ACTION, payload).unwrap();
    assert!(action.decode(BankVersion::V134).unwrap().is_some());

    assert_eq!(
        refusal(&legacy_with(action)),
        UpgradeRefusal::ActionClass {
            id: ObjectId(0x0BAD_F00D),
            class: 0x1500
        }
    );
}
