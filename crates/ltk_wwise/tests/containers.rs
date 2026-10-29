//! The fields of the containers through the public surface: decode, edit a child, encode, decode.

mod common;

use common::fixture;
use ltk_wwise::{
    bank::BankVersion,
    hierarchy::{
        Container, ContainerFields, ContainerMode, GroupType, LayerFields, NodeBase, Object,
        ObjectKind, PropRange, PropValue, PropertyId, SwitchFields, SwitchNodeParams, SwitchNodes,
        Typed,
    },
    Bank, ObjectId,
};

const V145: BankVersion = BankVersion::V145;

fn ids(ids: &[u32]) -> Vec<ObjectId> {
    ids.iter().copied().map(ObjectId).collect()
}

fn encode(container: &Container) -> Object {
    Typed::Container(container.clone())
        .encode(V145)
        .expect("encode")
}

fn decode(object: &Object) -> Container {
    match object.decode(V145).expect("decode") {
        Some(Typed::Container(container)) => container,
        other => panic!("not a container: {other:?}"),
    }
}

/// Returns the containers of the kind `kind` in the version 145 event bank fixture.
fn fixture_containers(kind: ObjectKind) -> Vec<Container> {
    let bank = Bank::from_reader(&mut &fixture("events_v145.bnk")[..]).expect("read bank");
    bank.hierarchy()
        .expect("hierarchy")
        .of_kind(kind)
        .map(decode)
        .collect()
}

/// Returns the node base of the ActorMixer of the version 145 event bank fixture.
fn node() -> NodeBase {
    fixture_containers(ObjectKind::ACTOR_MIXER).remove(0).node
}

/// Returns the bytes after the child IDs of `object`, a container with `children` children.
fn after_children(object: &Object, children: &[u32]) -> Vec<u8> {
    let payload = object.payload();
    let mut list = (children.len() as u32).to_le_bytes().to_vec();
    for id in children {
        list.extend_from_slice(&id.to_le_bytes());
    }
    let at = payload
        .windows(list.len())
        .rposition(|window| window == list)
        .expect("the child list");
    payload[at + list.len()..].to_vec()
}

#[test]
fn a_random_container_without_its_first_child_drops_it_from_the_playlist() {
    let containers = fixture_containers(ObjectKind::RANDOM_SEQUENCE_CONTAINER);
    assert!(!containers.is_empty());
    for container in containers {
        let ContainerFields::RandomSequence(fields) = &container.fields else {
            panic!("{}: not a random container", container.id);
        };
        assert!([ContainerMode::RANDOM, ContainerMode::SEQUENCE].contains(&fields.mode));
        assert_eq!(fields.playlist.len(), container.children.len());
        let first = container.children[0];

        let mut edited = container.clone();
        assert!(edited.remove_child(first));
        let decoded = decode(&encode(&edited));

        assert_eq!(decoded.children, container.children[1..]);
        let ContainerFields::RandomSequence(after) = &decoded.fields else {
            panic!("not a random container");
        };
        let expected: Vec<_> = fields
            .playlist
            .iter()
            .filter(|item| item.child != first)
            .copied()
            .collect();
        assert_eq!(after.playlist, expected);
        assert_eq!(after.mode, fields.mode);
        assert_eq!(decoded.node, container.node);
        assert_eq!(decoded, edited);
    }
}

#[test]
fn an_actor_mixer_without_a_child_keeps_its_other_children() {
    let mut mixer = fixture_containers(ObjectKind::ACTOR_MIXER).remove(0);
    let children = mixer.children.clone();
    assert!(mixer.remove_child(children[0]));
    assert!(!mixer.remove_child(ObjectId(0xDEAD_BEEF)));

    let decoded = decode(&encode(&mixer));

    assert_eq!(decoded.kind(), ObjectKind::ACTOR_MIXER);
    assert_eq!(decoded.children, children[1..]);
}

/// The fields of a SwitchContainer on the switch group 0x77 with three switches. The params of
/// each node have a fade-out time of 100 times its ID.
fn switch_fields() -> SwitchFields {
    let switches = [(1, &[10, 20][..]), (2, &[30]), (3, &[20, 30])];
    SwitchFields {
        group_type: GroupType::SWITCH,
        group: 0x77,
        default_switch: 2,
        continuous_validation: 1,
        switches: switches
            .into_iter()
            .map(|(switch, nodes)| SwitchNodes {
                switch,
                nodes: ids(nodes),
            })
            .collect(),
        params: [10, 20, 30]
            .into_iter()
            .map(|node| SwitchNodeParams {
                node: ObjectId(node),
                bits: 0,
                on_switch_bits: 0,
                fade_out_time: 100 * node as i32,
                fade_in_time: 0,
            })
            .collect(),
    }
}

/// The bytes of switch fields after the children: the grammar of section 9.7. Each param is a
/// node and its fade-out time.
fn switch_tail(switches: &[(u32, &[u32])], params: &[(u32, i32)]) -> Vec<u8> {
    let mut out = (switches.len() as u32).to_le_bytes().to_vec();
    for (switch, nodes) in switches {
        out.extend_from_slice(&switch.to_le_bytes());
        out.extend_from_slice(&(nodes.len() as u32).to_le_bytes());
        for node in *nodes {
            out.extend_from_slice(&node.to_le_bytes());
        }
    }
    out.extend_from_slice(&(params.len() as u32).to_le_bytes());
    for (node, fade_out) in params {
        out.extend_from_slice(&node.to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(&fade_out.to_le_bytes());
        out.extend_from_slice(&0i32.to_le_bytes());
    }
    out
}

fn switch_container() -> Container {
    Container {
        id: ObjectId(5),
        node: node(),
        children: ids(&[10, 20, 30]),
        fields: ContainerFields::Switch(switch_fields()),
    }
}

#[test]
fn a_switch_container_without_a_child_drops_it_from_its_switches_and_params() {
    let mut switch = switch_container();
    let object = encode(&switch);
    assert_eq!(object.kind(), ObjectKind::SWITCH_CONTAINER);
    assert_eq!(
        after_children(&object, &[10, 20, 30]),
        switch_tail(
            &[(1, &[10, 20]), (2, &[30]), (3, &[20, 30])],
            &[(10, 1000), (20, 2000), (30, 3000)]
        )
    );
    assert_eq!(decode(&object), switch);

    assert!(switch.remove_child(ObjectId(20)));
    let edited = encode(&switch);

    assert_eq!(
        after_children(&edited, &[10, 30]),
        switch_tail(
            &[(1, &[10]), (2, &[30]), (3, &[30])],
            &[(10, 1000), (30, 3000)]
        )
    );
    let decoded = decode(&edited);
    let ContainerFields::Switch(fields) = &decoded.fields else {
        panic!("not a switch container");
    };
    assert_eq!(fields.group_type, GroupType::SWITCH);
    assert_eq!(fields.group, 0x77);
    assert_eq!(fields.default_switch, 2);
    assert_eq!(fields.nodes_of(2), Some(&ids(&[30])[..]));
    assert_eq!(decoded.node, switch.node);
}

#[test]
fn a_switch_container_with_a_child_replaced_names_the_new_child_everywhere() {
    let mut switch = switch_container();

    assert!(switch.replace_child(ObjectId(20), ObjectId(40)));

    assert_eq!(
        after_children(&encode(&switch), &[10, 40, 30]),
        switch_tail(
            &[(1, &[10, 40]), (2, &[30]), (3, &[40, 30])],
            &[(10, 1000), (40, 2000), (30, 3000)]
        )
    );
}

/// A layer of the RTPC 0x99 with an empty RtpcList and an association of each child of
/// `children`, each with two curve points.
fn layer_bytes(id: u32, children: &[u32]) -> Vec<u8> {
    let mut out = id.to_le_bytes().to_vec();
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0x99u32.to_le_bytes());
    out.push(0);
    out.extend_from_slice(&(children.len() as u32).to_le_bytes());
    for child in children {
        out.extend_from_slice(&child.to_le_bytes());
        out.extend_from_slice(&2u32.to_le_bytes());
        for point in 0..2u32 {
            out.extend_from_slice(&(point as f32).to_le_bytes());
            out.extend_from_slice(&(*child as f32).to_le_bytes());
            out.extend_from_slice(&4u32.to_le_bytes());
        }
    }
    out
}

/// Returns a LayerContainer payload with the node base of the fixture ActorMixer.
fn layer_container(children: &[u32], layers: &[(u32, &[u32])]) -> Object {
    let mixer = encode(&Container {
        id: ObjectId(6),
        node: node(),
        children: Vec::new(),
        fields: ContainerFields::ActorMixer,
    });
    let mut payload = mixer.payload()[..mixer.payload().len() - 4].to_vec();
    payload.extend_from_slice(&(children.len() as u32).to_le_bytes());
    for child in children {
        payload.extend_from_slice(&child.to_le_bytes());
    }
    payload.extend_from_slice(&(layers.len() as u32).to_le_bytes());
    for (id, associated) in layers {
        payload.extend_from_slice(&layer_bytes(*id, associated));
    }
    payload.push(1);
    Object::new(ObjectKind::LAYER_CONTAINER, payload).expect("payload holds an ID")
}

#[test]
fn a_layer_container_without_a_child_drops_its_associations() {
    let object = layer_container(&[10, 20], &[(1, &[10, 20]), (2, &[20])]);
    let mut container = decode(&object);
    let ContainerFields::Layer(LayerFields { layers, .. }) = &container.fields else {
        panic!("not a layer container");
    };
    let associated: Vec<Vec<_>> = layers
        .iter()
        .map(|layer| layer.associations.iter().map(|a| a.child).collect())
        .collect();
    assert_eq!(associated, [ids(&[10, 20]), ids(&[20])]);
    assert_eq!(layers[0].rtpc, 0x99);
    assert_eq!(encode(&container), object);

    assert!(container.remove_child(ObjectId(20)));

    assert_eq!(
        encode(&container),
        layer_container(&[10], &[(1, &[10]), (2, &[])])
    );
}

#[test]
fn a_container_with_an_attenuation_and_a_pitch_range_decodes_with_both() {
    let mut container = fixture_containers(ObjectKind::RANDOM_SEQUENCE_CONTAINER).remove(0);
    let before = container.clone();
    container
        .node
        .props
        .set(PropertyId::ATTENUATION_ID, PropValue(0x1234_5678));
    let range = PropRange {
        min: PropValue::from_f32(-200.0),
        max: PropValue::from_f32(200.0),
    };
    container.node.ranged_props.set(PropertyId::PITCH, range);

    let decoded = decode(&encode(&container));

    assert_eq!(
        decoded.node.props.u32(PropertyId::ATTENUATION_ID),
        Some(0x1234_5678)
    );
    assert_eq!(
        decoded.node.ranged_props.f32(PropertyId::PITCH),
        Some((-200.0, 200.0))
    );
    for prop in &before.node.props {
        if prop.id != PropertyId::ATTENUATION_ID {
            assert_eq!(
                decoded.node.props.get(prop.id),
                Some(prop.value),
                "{}",
                prop.id
            );
        }
    }
    assert_eq!(decoded.children, before.children);
    assert_eq!(decoded.fields, before.fields);
}
