//! The event query through the public surface: events resolve to the media that they play.

mod common;

use common::fixture;
use ltk_wwise::{
    bank::{BankHeader, BankVersion, Section},
    hierarchy::{
        event_sources, event_tree, Action, Event, EventNode, EventSource, Hierarchy, Object,
        ObjectKind, StreamType, Typed,
    },
    Bank, BankId, Error, ObjectId, SourceId,
};

const V145: BankVersion = BankVersion::V145;

fn read(name: &str) -> Bank {
    Bank::from_reader(&mut &fixture(name)[..]).expect("read bank")
}

#[test]
fn an_event_id_that_no_bank_holds_resolves_to_an_empty_list() {
    let events = read("events_v145.bnk");
    let media = read("media_v145.bnk");

    let sources = event_sources(&[&events, &media], ObjectId(0xDEAD_BEEF)).expect("query");

    assert_eq!(sources, []);
}

#[test]
fn every_event_of_the_event_bank_resolves_to_media_that_the_media_bank_holds() {
    let events = read("events_v145.bnk");
    let media = read("media_v145.bnk");
    let index = media.media().expect("media index");

    let mut resolved = Vec::new();
    for event in events
        .hierarchy()
        .expect("hierarchy")
        .of_kind(ObjectKind::EVENT)
    {
        let sources = event_sources(&[&events, &media], event.id()).expect("query");
        for source in &sources {
            if source.stream_type == StreamType::DATA_BANK {
                assert!(index.contains(source.source), "{source:?}");
            }
        }
        let rows: Vec<_> = sources
            .iter()
            .map(|s| (s.sound.0, s.source.0, s.stream_type.0))
            .collect();
        resolved.push((event.id().0, rows));
    }

    assert!(resolved.iter().any(|(_, rows)| !rows.is_empty()));
    insta::assert_ron_snapshot!(resolved);
}

/// Returns a version 145 bank whose hierarchy holds `objects`.
fn bank(objects: impl IntoIterator<Item = Object>) -> Bank {
    let mut hierarchy = Hierarchy::new();
    for object in objects {
        hierarchy.push(object);
    }
    let mut bank = Bank::new(BankHeader::new(V145, BankId(1)));
    bank.sections_mut().push(Section::Hierarchy(hierarchy));
    bank
}

/// Returns the first object of the kind `kind` in the event bank fixture, decoded.
fn template(kind: ObjectKind) -> Typed {
    let fixture = read("events_v145.bnk");
    let object = fixture
        .hierarchy()
        .expect("hierarchy")
        .of_kind(kind)
        .next()
        .expect("an object of the kind");
    object
        .decode(V145)
        .expect("decode")
        .expect("a kind with a typed layout")
}

fn event(id: u32, actions: &[u32]) -> Object {
    let actions = actions.iter().copied().map(ObjectId).collect();
    let event = Event {
        id: ObjectId(id),
        actions,
    };
    Typed::Event(event).encode(V145).expect("encode")
}

fn play(id: u32, target: u32) -> Object {
    let action = Action::play(ObjectId(id), ObjectId(target), BankId(1));
    Typed::Action(action).encode(V145).expect("encode")
}

/// Returns a Stop action of `target`: type `0x0102`, empty property bundles, no exceptions.
fn stop(id: u32, target: u32) -> Object {
    let mut payload = id.to_le_bytes().to_vec();
    payload.extend_from_slice(&0x0102u16.to_le_bytes());
    payload.extend_from_slice(&target.to_le_bytes());
    payload.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
    Object::new(ObjectKind::ACTION, payload).expect("payload holds an ID")
}

fn sound(id: u32, source: u32) -> Object {
    let Typed::Sound(mut sound) = template(ObjectKind::SOUND) else {
        panic!("not a sound");
    };
    sound.id = ObjectId(id);
    sound.source.source_id = SourceId(source);
    Typed::Sound(sound).encode(V145).expect("encode")
}

fn mixer(id: u32, children: &[u32]) -> Object {
    let Typed::Container(mut mixer) = template(ObjectKind::ACTOR_MIXER) else {
        panic!("not a container");
    };
    mixer.id = ObjectId(id);
    mixer.children = children.iter().copied().map(ObjectId).collect();
    Typed::Container(mixer).encode(V145).expect("encode")
}

/// Returns the sound and source IDs of `sources`.
fn sounds_and_sources(sources: &[EventSource]) -> Vec<(u32, u32)> {
    sources.iter().map(|s| (s.sound.0, s.source.0)).collect()
}

#[test]
fn a_cycle_through_a_container_gives_each_source_once_in_depth_first_order() {
    let bank = bank([
        event(1, &[10]),
        play(10, 20),
        mixer(20, &[21, 30]),
        mixer(21, &[20, 30, 31]),
        sound(30, 300),
        sound(31, 310),
    ]);

    let sources = event_sources(&[&bank], ObjectId(1)).expect("query");

    assert_eq!(sounds_and_sources(&sources), [(30, 300), (31, 310)]);
}

#[test]
fn a_play_target_that_no_bank_holds_gives_nothing_and_the_other_actions_resolve() {
    let bank = bank([
        event(1, &[10, 11, 12]),
        play(10, 99),
        play(11, 30),
        sound(30, 300),
    ]);

    let sources = event_sources(&[&bank], ObjectId(1)).expect("query");

    assert_eq!(sounds_and_sources(&sources), [(30, 300)]);
}

#[test]
fn a_non_play_action_gives_nothing() {
    let bank = bank([
        event(1, &[10, 11]),
        stop(10, 30),
        play(11, 31),
        sound(30, 300),
        sound(31, 310),
    ]);

    let sources = event_sources(&[&bank], ObjectId(1)).expect("query");

    assert_eq!(sounds_and_sources(&sources), [(31, 310)]);
}

#[test]
fn a_non_play_action_that_does_not_decode_gives_nothing_and_does_not_stop_the_query() {
    let mut trailing_byte = stop(10, 30).into_payload();
    trailing_byte.push(0);
    let mut unknown_class = stop(11, 30).into_payload();
    unknown_class[4..6].copy_from_slice(&0x2a00u16.to_le_bytes());
    let bank = bank([
        event(1, &[10, 11, 12]),
        Object::new(ObjectKind::ACTION, trailing_byte).expect("payload holds an ID"),
        Object::new(ObjectKind::ACTION, unknown_class).expect("payload holds an ID"),
        play(12, 31),
        sound(30, 300),
        sound(31, 310),
    ]);

    let sources = event_sources(&[&bank], ObjectId(1)).expect("query");

    assert_eq!(sounds_and_sources(&sources), [(31, 310)]);
}

#[test]
fn the_first_object_of_an_id_in_a_bank_gives_its_media() {
    let bank = bank([
        event(1, &[10]),
        play(10, 30),
        sound(30, 300),
        sound(30, 999),
    ]);

    let sources = event_sources(&[&bank], ObjectId(1)).expect("query");

    assert_eq!(sounds_and_sources(&sources), [(30, 300)]);
}

#[test]
fn an_object_decodes_with_the_version_of_the_bank_that_holds_it() {
    let legacy = read("events_v134.bnk");
    let target = legacy
        .hierarchy()
        .expect("hierarchy")
        .of_kind(ObjectKind::SOUND)
        .next()
        .expect("a sound")
        .id();
    let Some(Typed::Sound(expected)) = legacy
        .hierarchy()
        .expect("hierarchy")
        .get(target)
        .expect("the sound")
        .decode(BankVersion::V134)
        .expect("decode")
    else {
        panic!("not a sound");
    };
    let current = bank([event(1, &[10]), play(10, target.0)]);

    let sources = event_sources(&[&current, &legacy], ObjectId(1)).expect("query");

    assert_eq!(
        sources,
        [EventSource {
            sound: target,
            source: expected.source.source_id,
            stream_type: expected.source.stream_type,
        }]
    );
}

#[test]
fn the_first_bank_that_holds_an_id_gives_its_object() {
    let first = bank([event(1, &[10]), play(10, 30), sound(30, 300)]);
    let second = bank([sound(30, 999)]);

    let sources = event_sources(&[&first, &second], ObjectId(1)).expect("query");

    assert_eq!(sounds_and_sources(&sources), [(30, 300)]);
}

#[test]
fn an_object_that_does_not_decode_stops_the_query_with_its_error() {
    let broken = Object::new(ObjectKind::SOUND, vec![30, 0, 0, 0, 1]).expect("payload holds an ID");
    let bank = bank([event(1, &[10]), play(10, 30), broken]);

    let error = event_sources(&[&bank], ObjectId(1)).expect_err("a layout error");

    assert!(
        matches!(
            error,
            Error::Layout {
                kind: ObjectKind::SOUND,
                id: ObjectId(30),
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn an_id_whose_object_is_not_an_event_resolves_to_an_empty_list() {
    let bank = bank([sound(30, 300)]);

    let sources = event_sources(&[&bank], ObjectId(30)).expect("query");

    assert_eq!(sources, []);
}

/// Appends the Sounds under `node` to `rows`, depth first, as (sound, source, stream type).
fn sound_leaves(node: &EventNode, rows: &mut Vec<(u32, u32, u8)>) {
    match node {
        EventNode::Sound(sound) => rows.push((
            sound.id.0,
            sound.source.source_id.0,
            sound.source.stream_type.0,
        )),
        EventNode::Container { children, .. } => {
            for child in children {
                sound_leaves(child, rows);
            }
        }
        EventNode::Other { .. } => {}
    }
}

#[test]
fn the_tree_of_every_event_of_the_event_bank_has_the_event_sources_snapshot_as_its_leaves() {
    let events = read("events_v145.bnk");
    let media = read("media_v145.bnk");

    let mut resolved = Vec::new();
    for event in events
        .hierarchy()
        .expect("hierarchy")
        .of_kind(ObjectKind::EVENT)
    {
        let tree = event_tree(&[&events, &media], event.id())
            .expect("query")
            .expect("an event");
        assert_eq!(tree.id, event.id());
        let mut rows = Vec::new();
        for target in tree.actions.iter().filter_map(|a| a.target.as_ref()) {
            sound_leaves(target, &mut rows);
        }
        resolved.push((event.id().0, rows));
    }

    insta::assert_ron_snapshot!(
        "every_event_of_the_event_bank_resolves_to_media_that_the_media_bank_holds",
        resolved
    );
}

#[test]
fn the_tree_of_an_event_holds_each_action_and_the_containers_and_sounds_under_a_play() {
    let bank = bank([
        event(1, &[10, 11]),
        play(10, 20),
        stop(11, 31),
        mixer(20, &[30, 21]),
        mixer(21, &[31]),
        sound(30, 300),
        sound(31, 310),
    ]);

    let tree = event_tree(&[&bank], ObjectId(1))
        .expect("query")
        .expect("an event");

    let [play, stop] = &tree.actions[..] else {
        panic!("{tree:?}");
    };
    assert!(play.action.action_type.is_play());
    assert_eq!(stop.action.action_type.class(), 0x0100);
    assert_eq!(stop.target, None);
    let Some(EventNode::Container {
        container,
        children,
    }) = &play.target
    else {
        panic!("{play:?}");
    };
    assert_eq!(container.id, ObjectId(20));
    let [EventNode::Sound(first), EventNode::Container {
        container: inner,
        children: inner_children,
    }] = &children[..]
    else {
        panic!("{children:?}");
    };
    assert_eq!(first.source.source_id, SourceId(300));
    assert_eq!(inner.id, ObjectId(21));
    let [EventNode::Sound(second)] = &inner_children[..] else {
        panic!("{inner_children:?}");
    };
    assert_eq!(second.id, ObjectId(31));
}

#[test]
fn the_tree_of_an_id_whose_object_is_not_an_event_is_none() {
    let bank = bank([sound(30, 300)]);

    assert_eq!(event_tree(&[&bank], ObjectId(30)).expect("query"), None);
    assert_eq!(event_tree(&[&bank], ObjectId(31)).expect("query"), None);
}

#[test]
fn a_node_of_a_kind_without_a_typed_layout_is_a_leaf_of_its_kind() {
    let attenuation = read("events_v145.bnk")
        .hierarchy()
        .expect("hierarchy")
        .of_kind(ObjectKind::ATTENUATION)
        .next()
        .expect("an attenuation")
        .clone();
    let id = attenuation.id();
    let bank = bank([
        event(1, &[10]),
        play(10, 20),
        mixer(20, &[id.0]),
        attenuation,
    ]);

    let tree = event_tree(&[&bank], ObjectId(1))
        .expect("query")
        .expect("an event");

    let Some(EventNode::Container { children, .. }) = &tree.actions[0].target else {
        panic!("{tree:?}");
    };
    assert_eq!(
        children[..],
        [EventNode::Other {
            id,
            kind: ObjectKind::ATTENUATION
        }]
    );
    assert_eq!(tree.sources().count(), 0);
}
