//! Hierarchies through the public surface: read, edit, write, read again.

mod common;

use common::fixture;
use ltk_wwise::{
    bank::Section,
    hierarchy::{Object, ObjectKind},
    Bank, Error, Hierarchy, ObjectId,
};

fn read(bytes: &[u8]) -> Bank {
    Bank::from_reader(&mut &bytes[..]).expect("read bank")
}

fn write(bank: &Bank) -> Vec<u8> {
    let mut out = Vec::new();
    bank.to_writer(&mut out).expect("write bank");
    out
}

/// Returns the `(kind, ID)` of each object of `hierarchy`, in order.
fn kinds_and_ids(hierarchy: &Hierarchy) -> Vec<(u8, u32)> {
    hierarchy
        .objects()
        .iter()
        .map(|object| (object.kind().0, object.id().0))
        .collect()
}

#[test]
fn event_banks_read_with_a_hierarchy_and_write_back_byte_for_byte() {
    for (name, len, first, last) in [
        ("events_v145.bnk", 33, (14, 546056065), (4, 3890987893)),
        ("events_v134.bnk", 8, (14, 362820833), (4, 1120491040)),
    ] {
        let bytes = fixture(name);
        let bank = read(&bytes);
        assert!(matches!(bank.sections(), [Section::Hierarchy(_)]), "{name}");
        let hierarchy = bank.hierarchy().expect(name);
        assert_eq!(hierarchy.len(), len, "{name}");
        let objects = kinds_and_ids(hierarchy);
        assert_eq!(objects.first(), Some(&first), "{name}");
        assert_eq!(objects.last(), Some(&last), "{name}");
        assert_eq!(write(&bank), bytes, "{name}");
    }
}

/// Returns an object of the kind `kind`, the ID `id` and one byte `tag` after the ID.
fn object(kind: ObjectKind, id: u32, tag: u8) -> Object {
    let mut payload = id.to_le_bytes().to_vec();
    payload.push(tag);
    Object::new(kind, payload).expect("payload holds an ID")
}

/// Returns the `(ID, tag)` of each object of `hierarchy`, in order.
fn ids_and_tags(hierarchy: &Hierarchy) -> Vec<(u32, u8)> {
    hierarchy
        .objects()
        .iter()
        .map(|object| (object.id().0, object.payload()[4]))
        .collect()
}

/// Returns a hierarchy of IDs 1, 2, 1, 3 in which the two objects of ID 1 differ.
fn with_a_repeated_id() -> Hierarchy {
    let mut hierarchy = Hierarchy::new();
    assert!(hierarchy.is_empty());
    hierarchy.push(object(ObjectKind::SOUND, 1, b'a'));
    hierarchy.push(object(ObjectKind::EVENT, 2, b'b'));
    hierarchy.push(object(ObjectKind::ACTION, 1, b'c'));
    hierarchy.push(object(ObjectKind::SOUND, 3, b'd'));
    hierarchy
}

#[test]
fn get_returns_the_first_object_of_an_id() {
    let mut hierarchy = with_a_repeated_id();
    assert_eq!(
        hierarchy.get(ObjectId(1)).map(Object::kind),
        Some(ObjectKind::SOUND)
    );
    assert_eq!(
        hierarchy.get(ObjectId(3)).map(Object::kind),
        Some(ObjectKind::SOUND)
    );
    assert!(hierarchy.get(ObjectId(4)).is_none());

    let first = hierarchy.get_mut(ObjectId(1)).expect("object 1");
    *first = object(ObjectKind::BUS, 1, b'e');
    assert_eq!(
        ids_and_tags(&hierarchy),
        [(1, b'e'), (2, b'b'), (1, b'c'), (3, b'd')]
    );
}

#[test]
fn of_kind_returns_the_objects_of_a_kind_in_file_order() {
    let hierarchy = with_a_repeated_id();
    let sounds: Vec<_> = hierarchy
        .of_kind(ObjectKind::SOUND)
        .map(|object| object.id().0)
        .collect();
    assert_eq!(sounds, [1, 3]);
    assert_eq!(hierarchy.of_kind(ObjectKind::STATE).count(), 0);
}

#[test]
fn replace_swaps_the_first_object_of_an_id_in_place_or_appends() {
    let mut hierarchy = with_a_repeated_id();
    let old = hierarchy.replace(object(ObjectKind::SOUND, 1, b'x'));
    assert_eq!(old, Some(object(ObjectKind::SOUND, 1, b'a')));
    assert_eq!(
        ids_and_tags(&hierarchy),
        [(1, b'x'), (2, b'b'), (1, b'c'), (3, b'd')]
    );

    assert_eq!(hierarchy.replace(object(ObjectKind::EVENT, 9, b'y')), None);
    assert_eq!(
        ids_and_tags(&hierarchy),
        [(1, b'x'), (2, b'b'), (1, b'c'), (3, b'd'), (9, b'y')]
    );
}

#[test]
fn remove_takes_out_the_first_object_of_an_id_and_keeps_the_order() {
    let mut hierarchy = with_a_repeated_id();
    let removed = hierarchy.remove(ObjectId(1)).map(Object::into_payload);
    assert_eq!(removed, Some(vec![1, 0, 0, 0, b'a']));
    assert_eq!(ids_and_tags(&hierarchy), [(2, b'b'), (1, b'c'), (3, b'd')]);
    assert_eq!(
        hierarchy.remove(ObjectId(1)).map(|o| o.kind()),
        Some(ObjectKind::ACTION)
    );
    assert!(hierarchy.remove(ObjectId(1)).is_none());
    assert_eq!(hierarchy.len(), 2);
}

#[test]
fn an_edited_hierarchy_writes_back_and_reads_with_the_edits() {
    let original = read(&fixture("events_v145.bnk"));
    let objects = original.hierarchy().unwrap().objects().to_vec();
    let removed = objects[2].id();
    let replaced = objects[3].id();
    let replacement = Object::new(
        ObjectKind::RANDOM_SEQUENCE_CONTAINER,
        [&replaced.0.to_le_bytes()[..], &[7; 9]].concat(),
    )
    .unwrap();
    let added = Object::new(ObjectKind(17), vec![0xaa, 0xbb, 0xcc, 0xdd]).unwrap();

    let mut bank = original.clone();
    let hierarchy = bank.hierarchy_mut().unwrap();
    assert!(hierarchy.remove(removed).is_some());
    assert!(hierarchy.replace(replacement.clone()).is_some());
    hierarchy.push(added.clone());

    let reread = read(&write(&bank));
    let mut expected = objects.clone();
    expected.remove(2);
    expected[2] = replacement;
    expected.push(added);
    assert_eq!(reread.hierarchy().unwrap().objects(), expected);
    assert_eq!(reread.hierarchy().unwrap().len(), 33);
    assert_eq!(
        reread.hierarchy().unwrap().get(ObjectId(0xddccbbaa)),
        Some(&expected[32])
    );
    assert_eq!(reread.header(), original.header());
}

#[test]
fn an_empty_hierarchy_writes_a_count_of_zero() {
    let mut bank = read(&fixture("empty_v145.bnk"));
    bank.sections_mut()
        .push(Section::Hierarchy(Hierarchy::new()));
    let bytes = write(&bank);
    assert_eq!(bytes[bytes.len() - 12..], *b"HIRC\x04\0\0\0\0\0\0\0");
    assert_eq!(read(&bytes).hierarchy(), Some(&Hierarchy::new()));
}

/// Returns the version 145 event bank with its `HIRC` body replaced by `body`.
fn with_hirc_body(body: &[u8]) -> Vec<u8> {
    let bytes = fixture("events_v145.bnk");
    let hirc_at = bytes.len() - 2051 - 8;
    assert_eq!(&bytes[hirc_at..hirc_at + 4], b"HIRC");
    let mut out = bytes[..hirc_at + 4].to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
    out
}

/// Returns a `HIRC` body of the count `count` and the objects `(kind, payload)`.
fn hirc_body(count: u32, objects: &[(u8, &[u8])]) -> Vec<u8> {
    let mut body = count.to_le_bytes().to_vec();
    for (kind, payload) in objects {
        body.push(*kind);
        body.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        body.extend_from_slice(payload);
    }
    body
}

fn read_err(bytes: &[u8]) -> Error {
    Bank::from_reader(&mut &bytes[..]).expect_err("bank must not read")
}

#[test]
fn the_reader_rejects_bytes_after_the_last_object() {
    let mut body = hirc_body(1, &[(2, &[1, 0, 0, 0])]);
    assert_eq!(read(&with_hirc_body(&body)).hierarchy().unwrap().len(), 1);
    body.push(0);
    match read_err(&with_hirc_body(&body)) {
        Error::Framing { offset, len } => assert_eq!((offset, len), (13, 14)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_reader_rejects_an_object_that_runs_past_the_chunk() {
    let mut body = hirc_body(2, &[(2, &[1, 0, 0, 0])]);
    match read_err(&with_hirc_body(&body)) {
        Error::Framing { offset, len } => assert_eq!((offset, len), (18, 13)),
        other => panic!("{other:?}"),
    }
    body.extend_from_slice(&[4, 8, 0, 0, 0, 2, 0, 0, 0]);
    match read_err(&with_hirc_body(&body)) {
        Error::Framing { offset, len } => assert_eq!((offset, len), (26, 22)),
        other => panic!("{other:?}"),
    }
    match read_err(&with_hirc_body(&[1, 0, 0])) {
        Error::Framing { offset, len } => assert_eq!((offset, len), (4, 3)),
        other => panic!("{other:?}"),
    }
    let huge = [&1u32.to_le_bytes()[..], &[2], &u32::MAX.to_le_bytes()].concat();
    assert!(matches!(
        read_err(&with_hirc_body(&huge)),
        Error::Framing { len: 9, .. }
    ));
}

#[test]
fn the_reader_rejects_a_payload_shorter_than_an_id() {
    match read_err(&with_hirc_body(&hirc_body(1, &[(2, &[1, 0, 0])]))) {
        Error::PayloadTooShort(len) => assert_eq!(len, 3),
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        Object::new(ObjectKind::SOUND, vec![0; 3]),
        Err(Error::PayloadTooShort(3))
    ));
    assert_eq!(
        Object::new(ObjectKind::SOUND, vec![5, 0, 0, 0])
            .unwrap()
            .id(),
        ObjectId(5)
    );
}

#[test]
fn object_kinds_display_the_names_of_their_constants() {
    let named = [
        (ObjectKind::STATE, "STATE"),
        (ObjectKind::SOUND, "SOUND"),
        (ObjectKind::ACTION, "ACTION"),
        (ObjectKind::EVENT, "EVENT"),
        (
            ObjectKind::RANDOM_SEQUENCE_CONTAINER,
            "RANDOM_SEQUENCE_CONTAINER",
        ),
        (ObjectKind::SWITCH_CONTAINER, "SWITCH_CONTAINER"),
        (ObjectKind::ACTOR_MIXER, "ACTOR_MIXER"),
        (ObjectKind::BUS, "BUS"),
        (ObjectKind::LAYER_CONTAINER, "LAYER_CONTAINER"),
        (ObjectKind::MUSIC_SEGMENT, "MUSIC_SEGMENT"),
        (ObjectKind::MUSIC_TRACK, "MUSIC_TRACK"),
        (ObjectKind::MUSIC_SWITCH_CONTAINER, "MUSIC_SWITCH_CONTAINER"),
        (
            ObjectKind::MUSIC_RANDOM_SEQUENCE_CONTAINER,
            "MUSIC_RANDOM_SEQUENCE_CONTAINER",
        ),
        (ObjectKind::ATTENUATION, "ATTENUATION"),
    ];
    for (value, (kind, name)) in (1..).zip(named) {
        assert_eq!(kind, ObjectKind(value));
        assert_eq!(kind.to_string(), name);
    }
    assert_eq!(ObjectKind(17).to_string(), "kind(17)");
    assert_eq!(ObjectKind(0).to_string(), "kind(0)");
}
