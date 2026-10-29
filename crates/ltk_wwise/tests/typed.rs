//! Typed hierarchy objects through the public surface: decode, edit, encode, decode again.

mod common;

use common::fixture;
use ltk_wwise::{
    bank::BankVersion,
    hierarchy::{
        Action, ActionType, Event, Object, ObjectKind, PropRange, PropValue, PropertyId, Typed,
    },
    Bank, BankId, Error, ObjectId, SourceId,
};

const V134: BankVersion = BankVersion::V134;
const V145: BankVersion = BankVersion::V145;

/// Returns the object that `typed` encodes to at `version`.
fn encode(typed: &Typed, version: BankVersion) -> Object {
    typed.encode(version).expect("encode")
}

/// Returns the typed object that `object` decodes to at `version`.
fn decode(object: &Object, version: BankVersion) -> Typed {
    object
        .decode(version)
        .expect("decode")
        .expect("a kind with a typed layout")
}

#[test]
fn an_event_round_trips_an_action_count_of_0_1_and_200() {
    for (count, varint, len) in [
        (0, &[0x00][..], 5),
        (1, &[0x01][..], 9),
        (200, &[0x81, 0x48][..], 806),
    ] {
        let event = Typed::Event(Event {
            id: ObjectId(0xAABB_CCDD),
            actions: (0..count).map(|n| ObjectId(1000 + n)).collect(),
        });
        for version in [V134, V145] {
            let object = encode(&event, version);
            assert_eq!(object.kind(), ObjectKind::EVENT);
            assert_eq!(object.id(), ObjectId(0xAABB_CCDD));
            assert_eq!(object.payload().len(), len, "{count} actions");
            assert_eq!(&object.payload()[4..4 + varint.len()], varint, "{count}");
            assert_eq!(decode(&object, version), event, "{count} actions");
        }
    }
}

fn read(name: &str) -> Bank {
    Bank::from_reader(&mut &fixture(name)[..]).expect("read bank")
}

fn write(bank: &Bank) -> Vec<u8> {
    let mut out = Vec::new();
    bank.to_writer(&mut out).expect("write bank");
    out
}

/// The kinds with a typed layout.
const TYPED_KINDS: [ObjectKind; 7] = [
    ObjectKind::SOUND,
    ObjectKind::ACTION,
    ObjectKind::EVENT,
    ObjectKind::RANDOM_SEQUENCE_CONTAINER,
    ObjectKind::SWITCH_CONTAINER,
    ObjectKind::ACTOR_MIXER,
    ObjectKind::LAYER_CONTAINER,
];

/// The event bank fixtures and the number of objects of a typed kind in each.
const EVENT_BANKS: [(&str, usize); 2] = [("events_v145.bnk", 31), ("events_v134.bnk", 7)];

#[test]
fn every_typed_object_of_the_event_banks_decodes_and_encodes_back_to_its_payload() {
    for (name, typed_count) in EVENT_BANKS {
        let bank = read(name);
        let version = bank.header().version;
        let mut decoded = 0;
        for object in bank.hierarchy().expect(name).objects() {
            let typed = object.decode(version).expect(name);
            assert_eq!(typed.is_some(), TYPED_KINDS.contains(&object.kind()));
            let Some(typed) = typed else { continue };
            decoded += 1;
            assert_eq!(typed.id(), object.id());
            assert_eq!(&encode(&typed, version), object, "{name} {}", object.id());
        }
        assert_eq!(decoded, typed_count, "{name}");
    }
}

#[test]
fn a_payload_with_a_byte_appended_or_cut_is_a_layout_error_at_the_end_of_the_layout() {
    for (name, _) in EVENT_BANKS {
        let bank = read(name);
        let version = bank.header().version;
        let objects = bank.hierarchy().expect(name).objects();
        for object in objects.iter().filter(|o| TYPED_KINDS.contains(&o.kind())) {
            let len = object.payload().len();
            let mut appended = object.payload().to_vec();
            appended.push(0);
            let cut = object.payload()[..len - 1].to_vec();
            for (payload, payload_len) in [(appended, len + 1), (cut, len - 1)] {
                let changed = Object::new(object.kind(), payload).expect("payload holds an ID");
                let error = changed.decode(version);
                assert!(
                    matches!(
                        error,
                        Err(Error::Layout { kind, id, offset, len: l })
                            if kind == object.kind() && id == object.id()
                                && offset == len && l == payload_len
                    ),
                    "{name} {} of {payload_len} bytes: {error:?}",
                    object.id()
                );
            }
        }
    }
}

#[test]
fn a_sound_with_a_new_source_id_replaces_its_object_and_the_bank_changes_only_that_payload() {
    let bytes = fixture("events_v145.bnk");
    let mut bank = read("events_v145.bnk");
    let version = bank.header().version;
    let hierarchy = bank.hierarchy_mut().expect("hierarchy");
    let original = hierarchy
        .of_kind(ObjectKind::SOUND)
        .next()
        .expect("a sound")
        .clone();
    let Typed::Sound(mut sound) = decode(&original, version) else {
        panic!("not a sound");
    };
    sound.source.source_id = SourceId(0x0102_0304);
    let replaced = hierarchy.replace(encode(&Typed::Sound(sound), version));
    assert_eq!(replaced.as_ref(), Some(&original));

    let out = write(&bank);
    assert_eq!(out.len(), bytes.len());
    let reread = Bank::from_reader(&mut &out[..]).expect("read bank");
    let before = read("events_v145.bnk");
    let pairs = before
        .hierarchy()
        .expect("hierarchy")
        .objects()
        .iter()
        .zip(reread.hierarchy().expect("hierarchy").objects());
    for (old, new) in pairs {
        if old.id() == original.id() {
            // The source ID follows the object ID, the plugin ID and the stream type.
            let mut expected = old.payload().to_vec();
            expected[9..13].copy_from_slice(&[4, 3, 2, 1]);
            assert_eq!(new.payload(), expected);
        } else {
            assert_eq!(new, old);
        }
    }
    let changed: Vec<_> = (0..out.len()).filter(|&at| out[at] != bytes[at]).collect();
    assert!(!changed.is_empty() && changed.len() <= 4, "{changed:?}");
}

#[test]
fn a_legacy_node_object_encodes_at_145_with_zero_metadata_and_a_zero_reflections_bus() {
    let bank = read("events_v134.bnk");
    let objects = bank.hierarchy().expect("hierarchy").objects();
    for object in objects.iter().filter(|o| {
        [
            ObjectKind::SOUND,
            ObjectKind::RANDOM_SEQUENCE_CONTAINER,
            ObjectKind::ACTOR_MIXER,
        ]
        .contains(&o.kind())
    }) {
        let legacy = decode(object, V134);
        let current = encode(&legacy, V145);
        let (old, new) = (object.payload(), current.payload());
        assert_eq!(new.len(), old.len() + 6, "{}", object.id());

        // The node base of a sound starts after the ID and a source of 14 bytes, the node base
        // of a container after the ID. Each fixture object has no FX.
        let node = if object.kind() == ObjectKind::SOUND {
            18
        } else {
            4
        };
        let fx_end = node + 2;
        assert_eq!(old[node + 1], 0, "fx count of {}", object.id());
        assert_eq!(new[..fx_end], old[..fx_end]);
        assert_eq!(new[fx_end..fx_end + 2], [0, 0]);
        let spliced = (fx_end..old.len()).any(|aux_end| {
            new[fx_end + 2..aux_end + 2] == old[fx_end..aux_end]
                && new[aux_end + 2..aux_end + 6] == [0; 4]
                && new[aux_end + 6..] == old[aux_end..]
        });
        assert!(spliced, "{}: no zero reflections bus", object.id());

        let decoded = decode(&current, V145);
        assert_eq!(decoded.id(), object.id());
        assert_eq!(&encode(&decoded, V134), object);
    }
}

#[test]
fn a_kind_without_a_typed_layout_decodes_to_none() {
    let bank = read("events_v145.bnk");
    let objects = bank.hierarchy().expect("hierarchy").objects();
    let attenuation = objects
        .iter()
        .find(|o| o.kind() == ObjectKind::ATTENUATION)
        .expect("an attenuation");
    assert!(matches!(attenuation.decode(V145), Ok(None)));
}

#[test]
fn a_play_action_encodes_to_18_bytes_at_134_and_22_at_145() {
    let play = Action::play(ObjectId(7), ObjectId(8), BankId(9));
    for (version, len) in [(V134, 18), (V145, 22)] {
        let object = encode(&Typed::Action(play.clone()), version);
        assert_eq!(object.kind(), ObjectKind::ACTION);
        assert_eq!(object.payload().len(), len, "{version}");
        let Typed::Action(action) = decode(&object, version) else {
            panic!("{version}: not an action");
        };
        assert!(action.action_type.is_play());
        assert_eq!(action.action_type, ActionType::PLAY);
        assert_eq!(action.id, ObjectId(7));
        assert_eq!(action.target, ObjectId(8));
        assert_eq!(action.play_bank(), Some(BankId(9)));
        assert_eq!(action, play);
    }
}

/// Returns an action payload of the type `action_type`, empty property bundles and `extension`.
fn action_payload(action_type: u16, extension: &[u8]) -> Object {
    let mut payload = 5u32.to_le_bytes().to_vec();
    payload.extend_from_slice(&action_type.to_le_bytes());
    payload.extend_from_slice(&6u32.to_le_bytes());
    payload.extend_from_slice(&[0, 0, 0]);
    payload.extend_from_slice(extension);
    Object::new(ObjectKind::ACTION, payload).expect("payload holds an ID")
}

#[test]
fn an_action_class_outside_the_generation_is_an_action_class_error() {
    for (action_type, version, extension) in [
        (0x3102, V134, &[0, 0, 0, 0, 0, 0, 0, 0][..]),
        (0x1500, V145, &[][..]),
        (0x4000, V145, &[][..]),
    ] {
        let error = action_payload(action_type, extension).decode(version);
        assert!(
            matches!(
                error,
                Err(Error::ActionClass { id: ObjectId(5), class }) if class == action_type & 0xFF00
            ),
            "{action_type:#06x} at {version}: {error:?}"
        );
    }
}

#[test]
fn a_legacy_only_class_decodes_at_134_and_does_not_encode_at_145() {
    let empty = action_payload(0x1500, &[]);
    let action = decode(&empty, V134);
    assert_eq!(encode(&action, V134), empty);
    assert!(matches!(
        action.encode(V145),
        Err(Error::ActionClass {
            id: ObjectId(5),
            class: 0x1500
        })
    ));
}

#[test]
fn a_version_without_layouts_is_unsupported() {
    let play = Typed::Action(Action::play(ObjectId(1), ObjectId(2), BankId(3)));
    let object = encode(&play, V145);
    for version in [BankVersion::V125, BankVersion(144), BankVersion(146)] {
        assert!(matches!(
            object.decode(version),
            Err(Error::UnsupportedLayout(v)) if v == version
        ));
        assert!(matches!(
            play.encode(version),
            Err(Error::UnsupportedLayout(v)) if v == version
        ));
    }
}

#[test]
fn a_sound_with_a_volume_set_decodes_with_that_volume_and_the_other_properties_unchanged() {
    let bank = read("events_v145.bnk");
    let object = bank
        .hierarchy()
        .expect("hierarchy")
        .of_kind(ObjectKind::SOUND)
        .next()
        .expect("a sound")
        .clone();
    let Typed::Sound(mut sound) = decode(&object, V145) else {
        panic!("not a sound");
    };
    let before = sound.node.props.clone();
    sound
        .node
        .props
        .set(PropertyId::VOLUME, PropValue::from_f32(-6.0));

    let Typed::Sound(edited) = decode(&encode(&Typed::Sound(sound.clone()), V145), V145) else {
        panic!("not a sound");
    };

    assert_eq!(edited.node.props.f32(PropertyId::VOLUME), Some(-6.0));
    for prop in before.iter().filter(|p| p.id != PropertyId::VOLUME) {
        assert_eq!(
            edited.node.props.get(prop.id),
            Some(prop.value),
            "{}",
            prop.id
        );
    }
    assert_eq!(edited, sound);
}

#[test]
fn a_play_action_with_a_delay_and_a_ranged_delay_decodes_with_both_at_134_and_145() {
    let mut play = Action::play(ObjectId(7), ObjectId(8), BankId(9));
    play.props
        .set(PropertyId::DELAY_TIME, PropValue::from_i32(250));
    let range = PropRange {
        min: PropValue::from_i32(-100),
        max: PropValue::from_i32(50),
    };
    play.ranged_props.set(PropertyId::DELAY_TIME, range);

    for (version, len) in [(V134, 18 + 5 + 9), (V145, 22 + 5 + 9)] {
        let object = encode(&Typed::Action(play.clone()), version);
        assert_eq!(object.payload().len(), len, "{version}");
        let Typed::Action(action) = decode(&object, version) else {
            panic!("{version}: not an action");
        };
        assert_eq!(action.props.i32(PropertyId::DELAY_TIME), Some(250));
        assert_eq!(
            action.ranged_props.i32(PropertyId::DELAY_TIME),
            Some((-100, 50))
        );
        assert_eq!(action.props.f32(PropertyId::DELAY_TIME), None);
        assert_eq!(action.play_bank(), Some(BankId(9)));
        assert_eq!(action, play);
    }
}
