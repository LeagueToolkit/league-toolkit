//! The fixture of `value-walk.md` section 7, walked twice: over the owned tree and over an
//! `ObjectView` of the same bytes, through one generic visitor.

use std::io;

use glam::{Mat4, Vec2, Vec3, Vec4};
use ltk_hash::BinHash;
use ltk_primitives::Color;

use super::{Leaf, Node, TreeNode, TreeValue, Visit, Visitor, WalkOutcome};
use crate::{
    concrete::{self, values, Bin, BinObject},
    property::values::{Embedded, UnorderedContainer},
    property::Kind,
    stream::ValueView,
    BinOverride, Error, PropertyValueEnum,
};

type BinStream = concrete::BinStream<io::Cursor<Vec<u8>>>;

const OBJECT: u32 = 0x0100_0001;
const C1: u32 = 0xC1A5_0001;
const C2: u32 = 0xC1A5_0002;
const C3: u32 = 0xC1A5_0003;
const C4: u32 = 0xC1A5_0004;
const C5: u32 = 0xC1A5_0005;
const C6: u32 = 0xC1A5_0006;
const C7: u32 = 0xC1A5_0007;
const C8: u32 = 0xC1A5_0008;
const C9: u32 = 0xC1A5_0009;

const F_STRUCT: u32 = 0x01;
const F_EMBED: u32 = 0x02;
const F_NULL_STRUCT: u32 = 0x03;
const F_NULL_EMBED: u32 = 0x04;
const F_CONT_STRUCT: u32 = 0x05;
const F_CONT_EMBED: u32 = 0x06;
const F_OPT_STRUCT: u32 = 0x07;
const F_OPT_NULL: u32 = 0x08;
const F_OPT_EMPTY: u32 = 0x09;
const F_MAP_STRUCT: u32 = 0x0A;
const F_MAP_EMBED: u32 = 0x0B;
const F_STRINGS: u32 = 0x0C;
const F_LEAF: u32 = 0x10;
const F_INNER: u32 = 0x11;
/// Leaf properties of the root: `F_LEAVES + kind as u32`.
const F_LEAVES: u32 = 0x100;
/// Maps of the root keyed by each valid key kind, holding `I32`: `F_KEYS + kind as u32`.
const F_KEYS: u32 = 0x200;

const KEY_A: u32 = 0x0000_00AA;
const KEY_B: u32 = 0x0000_00BB;

fn node(class: u32, properties: Vec<(u32, PropertyValueEnum)>) -> values::Struct {
    values::Struct {
        class_hash: class.into(),
        properties: properties
            .into_iter()
            .map(|(field, value)| (BinHash(field), value))
            .collect(),
        meta: Default::default(),
    }
}

fn null() -> values::Struct {
    values::Struct::default()
}

fn leaf_of(kind: Kind) -> PropertyValueEnum {
    use Kind as K;
    match kind {
        K::None => values::None::default().into(),
        K::Bool => values::Bool::new(true).into(),
        K::I8 => values::I8::new(-8).into(),
        K::U8 => values::U8::new(8).into(),
        K::I16 => values::I16::new(-16).into(),
        K::U16 => values::U16::new(16).into(),
        K::I32 => values::I32::new(-32).into(),
        K::U32 => values::U32::new(32).into(),
        K::I64 => values::I64::new(-64).into(),
        K::U64 => values::U64::new(64).into(),
        K::F32 => values::F32::new(1.5).into(),
        K::Vector2 => values::Vector2::new(Vec2::new(1.0, 2.0)).into(),
        K::Vector3 => values::Vector3::new(Vec3::new(1.0, 2.0, 3.0)).into(),
        K::Vector4 => values::Vector4::new(Vec4::new(1.0, 2.0, 3.0, 4.0)).into(),
        K::Matrix44 => values::Matrix44::new(Mat4::from_cols_array(&[
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
        ]))
        .into(),
        K::Color => values::Color::new(Color {
            r: 1u8,
            g: 2,
            b: 3,
            a: 4,
        })
        .into(),
        K::String => values::String::from("weapon").into(),
        K::Hash => values::Hash::new(0x1e6b_a0c4u32).into(),
        K::WadChunkLink => values::WadChunkLink::new(0x00c9_fd8f_1a2b_3c4du64).into(),
        K::ObjectLink => values::ObjectLink::new(0x0BEE_F000u32).into(),
        K::BitBool => values::BitBool::new(true).into(),
        _ => panic!("{kind:?} is not a leaf kind"),
    }
}

const LEAF_KINDS: [Kind; 21] = [
    Kind::None,
    Kind::Bool,
    Kind::I8,
    Kind::U8,
    Kind::I16,
    Kind::U16,
    Kind::I32,
    Kind::U32,
    Kind::I64,
    Kind::U64,
    Kind::F32,
    Kind::Vector2,
    Kind::Vector3,
    Kind::Vector4,
    Kind::Matrix44,
    Kind::Color,
    Kind::String,
    Kind::Hash,
    Kind::WadChunkLink,
    Kind::ObjectLink,
    Kind::BitBool,
];

fn key_kinds() -> impl Iterator<Item = Kind> {
    LEAF_KINDS.into_iter().filter(Kind::is_valid_map_key)
}

/// The section 7 fixture: a `Struct` and an `Embedded` at a property, inside a container,
/// inside an optional, as a map value; a null pointer in each position; a container of
/// strings; a map keyed by every kind `Kind::is_valid_map_key` admits; one leaf of every kind.
fn fixture() -> Bin {
    let mut object = BinObject::builder(OBJECT, C1)
        .property(
            F_STRUCT,
            node(
                C2,
                vec![
                    (F_LEAF, values::I32::new(1).into()),
                    (
                        F_INNER,
                        Embedded(node(C9, vec![(F_LEAF, leaf_of(Kind::F32))])).into(),
                    ),
                ],
            ),
        )
        .property(
            F_EMBED,
            Embedded(node(C3, vec![(F_LEAF, leaf_of(Kind::String))])),
        )
        .property(F_NULL_STRUCT, null())
        .property(F_NULL_EMBED, Embedded(null()))
        .property(
            F_CONT_STRUCT,
            values::Container::from(vec![node(C4, vec![]), null(), node(C4, vec![])]),
        )
        .property(
            F_CONT_EMBED,
            UnorderedContainer(values::Container::from(vec![
                Embedded(node(C5, vec![])),
                Embedded(null()),
            ])),
        )
        .property(
            F_OPT_STRUCT,
            values::Optional::new(Kind::Struct, Some(node(C6, vec![]).into())).unwrap(),
        )
        .property(
            F_OPT_NULL,
            values::Optional::new(Kind::Struct, Some(null().into())).unwrap(),
        )
        .property(
            F_OPT_EMPTY,
            values::Optional::empty(Kind::Embedded).unwrap(),
        )
        .property(
            F_MAP_STRUCT,
            values::Map::new(
                Kind::Hash,
                Kind::Struct,
                vec![
                    (values::Hash::new(KEY_A).into(), node(C7, vec![]).into()),
                    (values::Hash::new(KEY_B).into(), null().into()),
                ],
            )
            .unwrap(),
        )
        .property(
            F_MAP_EMBED,
            values::Map::new(
                Kind::String,
                Kind::Embedded,
                vec![(
                    values::String::from("k").into(),
                    Embedded(node(C8, vec![])).into(),
                )],
            )
            .unwrap(),
        )
        .property(
            F_STRINGS,
            values::Container::from(vec![values::String::from("a"), values::String::from("b")]),
        )
        .build();

    for kind in LEAF_KINDS {
        object
            .properties
            .insert(BinHash(F_LEAVES + kind as u32), leaf_of(kind));
    }
    for kind in key_kinds() {
        let map = values::Map::new(
            kind,
            Kind::I32,
            vec![(leaf_of(kind), values::I32::new(7).into())],
        )
        .unwrap();
        object
            .properties
            .insert(BinHash(F_KEYS + kind as u32), map.into());
    }

    Bin::builder().object(object).build()
}

fn bytes_of(bin: &Bin) -> Vec<u8> {
    let mut cursor = io::Cursor::new(Vec::new());
    bin.to_writer(&mut cursor).expect("the bin writes");
    cursor.into_inner()
}

/// Every node of the fixture: `(trail, class)`, in pre-order.
const EXPECTED_NODES: [(&str, u32); 10] = [
    ("", C1),
    ("00000001", C2),
    ("00000001.00000011", C9),
    ("00000002", C3),
    ("00000005[0]", C4),
    ("00000005[2]", C4),
    ("00000006[0]", C5),
    ("00000007[0]", C6),
    ("0000000a{000000aa}", C7),
    ("0000000b{\"k\"}", C8),
];

#[derive(Debug, Clone, PartialEq)]
enum Event {
    EnterNode {
        object: u32,
        class: u32,
        trail: String,
        classes: Vec<u32>,
    },
    ExitNode {
        class: u32,
        trail: String,
    },
    EnterProperty {
        field: u32,
        trail: String,
        holds_node: bool,
    },
    ExitProperty {
        field: u32,
        trail: String,
    },
}

impl Event {
    fn is_enter_node(&self) -> bool {
        matches!(self, Event::EnterNode { .. })
    }
}

/// Records every callback and answers each with `answer`.
struct Recorder {
    events: Vec<Event>,
    answer: fn(&Event) -> Result<Visit, Error>,
}

impl Recorder {
    fn new(answer: fn(&Event) -> Result<Visit, Error>) -> Self {
        Self {
            events: Vec::new(),
            answer,
        }
    }

    fn record(&mut self, event: Event) -> Result<Visit, Error> {
        let visit = (self.answer)(&event);
        self.events.push(event);
        visit
    }

    fn nodes(&self) -> Vec<(String, u32)> {
        self.events
            .iter()
            .filter_map(|e| match e {
                Event::EnterNode { trail, class, .. } => Some((trail.clone(), *class)),
                _ => None,
            })
            .collect()
    }
}

fn hashes(classes: &[BinHash]) -> Vec<u32> {
    classes.iter().map(|h| h.0).collect()
}

impl<'a, V: TreeValue<'a>> Visitor<'a, V> for Recorder {
    type Error = Error;

    fn enter_node(&mut self, node: &Node<'_, 'a, V>) -> Result<Visit, Error> {
        self.record(Event::EnterNode {
            object: node.object_hash().0,
            class: node.class_hash().0,
            trail: node.trail().to_string(),
            classes: hashes(node.trail().classes()),
        })
    }

    fn exit_node(&mut self, node: &Node<'_, 'a, V>) -> Result<Visit, Error> {
        self.record(Event::ExitNode {
            class: node.class_hash().0,
            trail: node.trail().to_string(),
        })
    }

    fn enter_property(
        &mut self,
        field: BinHash,
        value: V,
        node: &Node<'_, 'a, V>,
    ) -> Result<Visit, Error> {
        self.record(Event::EnterProperty {
            field: field.0,
            trail: node.trail().to_string(),
            holds_node: value.holds_node()?,
        })
    }

    fn exit_property(
        &mut self,
        field: BinHash,
        _value: V,
        node: &Node<'_, 'a, V>,
    ) -> Result<Visit, Error> {
        self.record(Event::ExitProperty {
            field: field.0,
            trail: node.trail().to_string(),
        })
    }
}

/// Runs one visitor over the owned tree and another over the view of the same bytes.
fn walk_both<W>(bin: &Bin, make: impl Fn() -> W) -> [(W, Result<WalkOutcome, Error>); 2]
where
    W: for<'a> Visitor<'a, &'a PropertyValueEnum, Error = Error>
        + for<'a> Visitor<'a, ValueView<'a>, Error = Error>,
{
    let mut owned = make();
    let owned_outcome = bin.walk(&mut owned);

    let mut viewed = make();
    let mut stream = BinStream::mount(io::Cursor::new(bytes_of(bin))).expect("the stream mounts");
    let viewed_outcome = stream.walk(&mut viewed);

    [(owned, owned_outcome), (viewed, viewed_outcome)]
}

fn always_continue(_: &Event) -> Result<Visit, Error> {
    Ok(Visit::Continue)
}

fn record_both(bin: &Bin, answer: fn(&Event) -> Result<Visit, Error>) -> [Recorder; 2] {
    let [(owned, a), (viewed, b)] = walk_both(bin, || Recorder::new(answer));
    assert_eq!(
        a.as_ref().ok(),
        b.as_ref().ok(),
        "the two trees ended differently"
    );
    assert_eq!(owned.events, viewed.events, "the two trees differ");
    [owned, viewed]
}

#[test]
fn visits_every_node_in_pre_order_over_both_trees() {
    let [owned, _] = record_both(&fixture(), always_continue);
    let expected: Vec<_> = EXPECTED_NODES
        .iter()
        .map(|(trail, class)| ((*trail).to_owned(), *class))
        .collect();
    assert_eq!(owned.nodes(), expected);
    assert!(owned
        .events
        .iter()
        .all(|e| !matches!(e, Event::EnterNode { object, .. } if *object != OBJECT)));
}

#[test]
fn a_null_pointer_is_never_visited() {
    let [owned, _] = record_both(&fixture(), always_continue);
    assert!(!owned
        .events
        .iter()
        .any(|e| matches!(e, Event::EnterNode { class: 0, .. })));
    // The null positions: `F_NULL_STRUCT`, `F_NULL_EMBED`, `[1]` of the container, `[1]` of the
    // unordered container, the optional, and `KEY_B` of the map.
    for absent in [
        "00000003",
        "00000004",
        "00000005[1]",
        "00000006[1]",
        "00000008[0]",
        "0000000a{000000bb}",
    ] {
        assert!(
            !owned.nodes().iter().any(|(trail, _)| trail == absent),
            "{absent} was visited"
        );
    }
}

#[test]
fn skip_at_a_property_prunes_only_what_is_beneath_it() {
    fn skip_struct(event: &Event) -> Result<Visit, Error> {
        Ok(match event {
            Event::EnterProperty {
                field: F_STRUCT, ..
            } => Visit::Skip,
            _ => Visit::Continue,
        })
    }
    let [owned, _] = record_both(&fixture(), skip_struct);
    let expected: Vec<_> = EXPECTED_NODES
        .iter()
        .filter(|(trail, _)| !trail.starts_with("00000001"))
        .map(|(trail, class)| ((*trail).to_owned(), *class))
        .collect();
    assert_eq!(owned.nodes(), expected);
    assert!(owned.events.contains(&Event::ExitProperty {
        field: F_STRUCT,
        trail: String::new(),
    }));
}

#[test]
fn exits_pair_with_entries_and_a_leaf_has_none() {
    let [owned, _] = record_both(&fixture(), always_continue);

    let entered_nodes = owned.events.iter().filter(|e| e.is_enter_node()).count();
    let exited_nodes = owned
        .events
        .iter()
        .filter(|e| matches!(e, Event::ExitNode { .. }))
        .count();
    assert_eq!(entered_nodes, exited_nodes);
    assert_eq!(entered_nodes, EXPECTED_NODES.len());

    let mut descended: Vec<_> = owned
        .events
        .iter()
        .filter_map(|e| match e {
            Event::EnterProperty {
                field,
                trail,
                holds_node: true,
            } => Some((*field, trail.clone())),
            _ => None,
        })
        .collect();
    let mut exited: Vec<_> = owned
        .events
        .iter()
        .filter_map(|e| match e {
            Event::ExitProperty { field, trail } => Some((*field, trail.clone())),
            _ => None,
        })
        .collect();
    descended.sort();
    exited.sort();
    assert_eq!(descended, exited);
    // The empty optional and the leaves of every kind are entered and never exited.
    assert!(descended.contains(&(F_OPT_EMPTY, String::new())));
    assert!(!exited.iter().any(|(field, _)| *field == F_STRINGS));
    assert!(!exited.iter().any(|(field, _)| *field >= F_LEAVES));

    // Well nested: every exit closes the innermost open entry.
    let mut open: Vec<&Event> = Vec::new();
    for event in &owned.events {
        match event {
            Event::EnterNode { .. } => open.push(event),
            Event::EnterProperty {
                holds_node: true, ..
            } => open.push(event),
            Event::EnterProperty { .. } => {}
            Event::ExitNode { class, trail } => {
                let Some(Event::EnterNode {
                    class: c, trail: t, ..
                }) = open.pop()
                else {
                    panic!("exit_node without an open node");
                };
                assert_eq!((c, t), (class, trail));
            }
            Event::ExitProperty { field, trail } => {
                let Some(Event::EnterProperty {
                    field: f, trail: t, ..
                }) = open.pop()
                else {
                    panic!("exit_property without an open property");
                };
                assert_eq!((f, t), (field, trail));
            }
        }
    }
    assert!(open.is_empty());
}

#[test]
fn skip_from_enter_node_skips_its_properties_and_still_exits() {
    fn skip_c2(event: &Event) -> Result<Visit, Error> {
        Ok(match event {
            Event::EnterNode { class: C2, .. } => Visit::Skip,
            _ => Visit::Continue,
        })
    }
    let [owned, _] = record_both(&fixture(), skip_c2);
    let nodes = owned.nodes();
    assert!(!nodes.iter().any(|(_, class)| *class == C9));
    assert!(!owned
        .events
        .iter()
        .any(|e| matches!(e, Event::EnterProperty { trail, .. } if trail == "00000001")));
    assert!(owned.events.contains(&Event::ExitNode {
        class: C2,
        trail: "00000001".into(),
    }));
    assert_eq!(nodes.len(), EXPECTED_NODES.len() - 1);
}

#[test]
fn skip_from_exit_property_prunes_the_remaining_properties() {
    fn skip_after_struct(event: &Event) -> Result<Visit, Error> {
        Ok(match event {
            Event::ExitProperty {
                field: F_STRUCT, ..
            } => Visit::Skip,
            _ => Visit::Continue,
        })
    }
    let [owned, _] = record_both(&fixture(), skip_after_struct);
    assert_eq!(
        owned.nodes(),
        [
            (String::new(), C1),
            ("00000001".to_owned(), C2),
            ("00000001.00000011".to_owned(), C9),
        ]
    );
    let root_properties = owned
        .events
        .iter()
        .filter(|e| matches!(e, Event::EnterProperty { trail, .. } if trail.is_empty()))
        .count();
    assert_eq!(root_properties, 1);
    assert_eq!(
        owned.events.last(),
        Some(&Event::ExitNode {
            class: C1,
            trail: String::new(),
        })
    );
}

#[test]
fn skip_from_exit_node_prunes_the_parent_propertys_remaining_items() {
    fn skip_after_first_c4(event: &Event) -> Result<Visit, Error> {
        Ok(match event {
            Event::ExitNode { trail, .. } if trail == "00000005[0]" => Visit::Skip,
            _ => Visit::Continue,
        })
    }
    let [owned, _] = record_both(&fixture(), skip_after_first_c4);
    let nodes = owned.nodes();
    assert!(!nodes.iter().any(|(trail, _)| trail == "00000005[2]"));
    assert!(nodes.iter().any(|(trail, _)| trail == "00000006[0]"));
    let position = |event: &Event| owned.events.iter().position(|e| e == event).unwrap();
    let exit_c4 = position(&Event::ExitNode {
        class: C4,
        trail: "00000005[0]".into(),
    });
    let exit_property = position(&Event::ExitProperty {
        field: F_CONT_STRUCT,
        trail: String::new(),
    });
    assert_eq!(exit_property, exit_c4 + 1);
}

#[test]
fn stop_unwinds_every_open_exit_and_reports_stopped() {
    fn stop_at_c9(event: &Event) -> Result<Visit, Error> {
        Ok(match event {
            Event::EnterNode { class: C9, .. } => Visit::Stop,
            _ => Visit::Continue,
        })
    }
    let [(owned, outcome), (_, viewed_outcome)] =
        walk_both(&fixture(), || Recorder::new(stop_at_c9));
    assert_eq!(outcome.unwrap(), WalkOutcome::Stopped);
    assert_eq!(viewed_outcome.unwrap(), WalkOutcome::Stopped);
    let at = owned
        .events
        .iter()
        .position(|e| matches!(e, Event::EnterNode { class: C9, .. }))
        .unwrap();
    assert_eq!(
        &owned.events[at + 1..],
        [
            Event::ExitNode {
                class: C9,
                trail: "00000001.00000011".into(),
            },
            Event::ExitProperty {
                field: F_INNER,
                trail: "00000001".into(),
            },
            Event::ExitNode {
                class: C2,
                trail: "00000001".into(),
            },
            Event::ExitProperty {
                field: F_STRUCT,
                trail: String::new(),
            },
            Event::ExitNode {
                class: C1,
                trail: String::new(),
            },
        ]
    );
}

#[test]
fn stop_from_enter_property_exits_a_property_that_holds_a_node() {
    fn stop_at_struct(event: &Event) -> Result<Visit, Error> {
        Ok(match event {
            Event::EnterProperty {
                field: F_STRUCT, ..
            } => Visit::Stop,
            _ => Visit::Continue,
        })
    }
    let [(owned, outcome), _] = walk_both(&fixture(), || Recorder::new(stop_at_struct));
    assert_eq!(outcome.unwrap(), WalkOutcome::Stopped);
    assert_eq!(
        &owned.events[2..],
        [
            Event::ExitProperty {
                field: F_STRUCT,
                trail: String::new(),
            },
            Event::ExitNode {
                class: C1,
                trail: String::new(),
            },
        ]
    );
}

#[test]
fn abort_runs_no_further_callback_and_reports_aborted() {
    fn abort_at_c9(event: &Event) -> Result<Visit, Error> {
        Ok(match event {
            Event::EnterNode { class: C9, .. } => Visit::Abort,
            _ => Visit::Continue,
        })
    }
    let [(owned, outcome), (viewed, viewed_outcome)] =
        walk_both(&fixture(), || Recorder::new(abort_at_c9));
    assert_eq!(outcome.unwrap(), WalkOutcome::Aborted);
    assert_eq!(viewed_outcome.unwrap(), WalkOutcome::Aborted);
    assert!(matches!(
        owned.events.last(),
        Some(Event::EnterNode { class: C9, .. })
    ));
    assert_eq!(owned.events, viewed.events);
}

#[test]
fn a_visitor_error_ends_the_walk_like_an_abort() {
    fn fail_at_c9(event: &Event) -> Result<Visit, Error> {
        match event {
            Event::EnterNode { class: C9, .. } => Err(Error::EmptyContainer),
            _ => Ok(Visit::Continue),
        }
    }
    let [(owned, outcome), (viewed, viewed_outcome)] =
        walk_both(&fixture(), || Recorder::new(fail_at_c9));
    assert!(matches!(outcome, Err(Error::EmptyContainer)));
    assert!(matches!(viewed_outcome, Err(Error::EmptyContainer)));
    assert!(matches!(
        owned.events.last(),
        Some(Event::EnterNode { class: C9, .. })
    ));
    assert_eq!(owned.events, viewed.events);
}

/// Collects `to_struct` at every node and `leaf` and `to_value` at every property.
#[derive(Default)]
struct Materialiser {
    structs: Vec<(String, values::Struct)>,
    leaves: Vec<(u32, Option<Leaf<'static>>)>,
    values: Vec<(u32, PropertyValueEnum)>,
}

fn owned_leaf(leaf: Leaf<'_>) -> Leaf<'static> {
    match leaf {
        Leaf::String(s) => Leaf::String(Box::leak(s.to_owned().into_boxed_str())),
        Leaf::None => Leaf::None,
        Leaf::Bool(v) => Leaf::Bool(v),
        Leaf::I8(v) => Leaf::I8(v),
        Leaf::U8(v) => Leaf::U8(v),
        Leaf::I16(v) => Leaf::I16(v),
        Leaf::U16(v) => Leaf::U16(v),
        Leaf::I32(v) => Leaf::I32(v),
        Leaf::U32(v) => Leaf::U32(v),
        Leaf::I64(v) => Leaf::I64(v),
        Leaf::U64(v) => Leaf::U64(v),
        Leaf::F32(v) => Leaf::F32(v),
        Leaf::Vector2(v) => Leaf::Vector2(v),
        Leaf::Vector3(v) => Leaf::Vector3(v),
        Leaf::Vector4(v) => Leaf::Vector4(v),
        Leaf::Matrix44(v) => Leaf::Matrix44(v),
        Leaf::Color(v) => Leaf::Color(v),
        Leaf::Hash(v) => Leaf::Hash(v),
        Leaf::File(v) => Leaf::File(v),
        Leaf::Link(v) => Leaf::Link(v),
        Leaf::Flag(v) => Leaf::Flag(v),
    }
}

impl<'a, V: TreeValue<'a>> Visitor<'a, V> for Materialiser {
    type Error = Error;

    fn enter_node(&mut self, node: &Node<'_, 'a, V>) -> Result<Visit, Error> {
        self.structs
            .push((node.trail().to_string(), node.inner().to_struct()?));
        Ok(Visit::Continue)
    }

    fn enter_property(
        &mut self,
        field: BinHash,
        value: V,
        node: &Node<'_, 'a, V>,
    ) -> Result<Visit, Error> {
        if node.is_root() {
            self.leaves.push((field.0, value.leaf()?.map(owned_leaf)));
            self.values.push((field.0, value.to_value()?));
        }
        Ok(Visit::Continue)
    }
}

#[test]
fn to_struct_equals_the_eager_parse_on_a_root_and_a_nested_node() {
    let bin = fixture();
    let object = &bin.objects[&BinHash(OBJECT)];
    let [(owned, _), (viewed, _)] = walk_both(&bin, Materialiser::default);
    assert_eq!(owned.structs, viewed.structs);

    let (root_trail, root) = &owned.structs[0];
    assert!(root_trail.is_empty());
    assert_eq!(root.class_hash.0, C1);
    assert_eq!(root.properties, object.properties);

    let (nested_trail, nested) = &owned.structs[1];
    assert_eq!(nested_trail, "00000001");
    assert_eq!(
        PropertyValueEnum::Struct(nested.clone()),
        object.properties[&BinHash(F_STRUCT)]
    );
}

#[test]
fn leaves_and_values_agree_between_the_trees_for_every_kind() {
    let bin = fixture();
    let object = &bin.objects[&BinHash(OBJECT)];
    let [(owned, _), (viewed, _)] = walk_both(&bin, Materialiser::default);
    assert_eq!(owned.leaves, viewed.leaves);
    assert_eq!(owned.values, viewed.values);

    for kind in LEAF_KINDS {
        let field = F_LEAVES + kind as u32;
        let (_, leaf) = owned.leaves.iter().find(|(f, _)| *f == field).unwrap();
        let leaf = leaf.expect("a leaf kind decodes");
        assert_eq!(leaf.kind(), kind);
    }
    assert_eq!(
        owned
            .leaves
            .iter()
            .filter(|(f, leaf)| *f < F_LEAVES && leaf.is_some())
            .count(),
        0,
        "a complex kind is not a leaf"
    );
    for (field, value) in &owned.values {
        assert_eq!(value, &object.properties[&BinHash(*field)], "{field:x}");
    }
}

#[test]
fn map_keys_of_every_kind_agree_between_the_trees() {
    let bin = fixture();
    let [(owned, _), (viewed, _)] = walk_both(&bin, Materialiser::default);
    for kind in key_kinds() {
        let field = F_KEYS + kind as u32;
        let find = |m: &Materialiser| {
            m.values
                .iter()
                .find(|(f, _)| *f == field)
                .unwrap()
                .1
                .clone()
        };
        let (a, b) = (find(&owned), find(&viewed));
        assert_eq!(a, b, "{kind:?}");
        let PropertyValueEnum::Map(map) = a else {
            panic!("{kind:?}: not a map")
        };
        assert_eq!(map.key_kind(), kind);
        assert_eq!(map.entries()[0].0, leaf_of(kind));
    }
}

#[test]
fn the_class_context_holds_the_class_of_every_enclosing_node() {
    let [owned, _] = record_both(&fixture(), always_continue);
    // A field step is read on exactly one node: the context is the open nodes, root first.
    let mut open: Vec<u32> = Vec::new();
    for event in &owned.events {
        match event {
            Event::EnterNode {
                class,
                classes,
                trail,
                ..
            } => {
                assert_eq!(classes, &open, "{trail}");
                open.push(*class);
            }
            Event::ExitNode { .. } => {
                open.pop();
            }
            _ => {}
        }
    }
    assert!(owned.events.contains(&Event::EnterNode {
        object: OBJECT,
        class: C9,
        trail: "00000001.00000011".into(),
        classes: vec![C1, C2],
    }));
}

#[test]
fn stop_on_a_leaf_property_exits_nothing_for_it() {
    fn stop_at_strings(event: &Event) -> Result<Visit, Error> {
        Ok(match event {
            Event::EnterProperty {
                field: F_STRINGS, ..
            } => Visit::Stop,
            _ => Visit::Continue,
        })
    }
    let [(owned, outcome), _] = walk_both(&fixture(), || Recorder::new(stop_at_strings));
    assert_eq!(outcome.unwrap(), WalkOutcome::Stopped);
    let at = owned
        .events
        .iter()
        .position(|e| {
            matches!(
                e,
                Event::EnterProperty {
                    field: F_STRINGS,
                    ..
                }
            )
        })
        .unwrap();
    assert_eq!(
        &owned.events[at + 1..],
        [Event::ExitNode {
            class: C1,
            trail: String::new(),
        }]
    );
}

#[test]
fn stop_and_abort_from_an_exit_end_the_walk() {
    fn stop_at_exit_property(event: &Event) -> Result<Visit, Error> {
        Ok(match event {
            Event::ExitProperty { field: F_INNER, .. } => Visit::Stop,
            _ => Visit::Continue,
        })
    }
    let [(owned, outcome), _] = walk_both(&fixture(), || Recorder::new(stop_at_exit_property));
    assert_eq!(outcome.unwrap(), WalkOutcome::Stopped);
    let at = owned
        .events
        .iter()
        .position(|e| matches!(e, Event::ExitProperty { field: F_INNER, .. }))
        .unwrap();
    assert_eq!(
        &owned.events[at + 1..],
        [
            Event::ExitNode {
                class: C2,
                trail: "00000001".into(),
            },
            Event::ExitProperty {
                field: F_STRUCT,
                trail: String::new(),
            },
            Event::ExitNode {
                class: C1,
                trail: String::new(),
            },
        ]
    );

    fn abort_at_exit_node(event: &Event) -> Result<Visit, Error> {
        Ok(match event {
            Event::ExitNode { class: C9, .. } => Visit::Abort,
            _ => Visit::Continue,
        })
    }
    let [(owned, outcome), _] = walk_both(&fixture(), || Recorder::new(abort_at_exit_node));
    assert_eq!(outcome.unwrap(), WalkOutcome::Aborted);
    assert!(matches!(
        owned.events.last(),
        Some(Event::ExitNode { class: C9, .. })
    ));
}

/// A map keyed by every kind, to a node, for the hash form of each key.
fn keyed_fixture() -> Bin {
    let mut object = BinObject::builder(OBJECT, C1).build();
    for kind in key_kinds() {
        let map = values::Map::new(
            kind,
            Kind::Struct,
            vec![(leaf_of(kind), node(C7, vec![]).into())],
        )
        .unwrap();
        object
            .properties
            .insert(BinHash(F_KEYS + kind as u32), map.into());
    }
    Bin::builder().object(object).build()
}

#[test]
fn the_hash_form_renders_every_key_kind() {
    let [owned, _] = record_both(&keyed_fixture(), always_continue);
    let expected = [
        (Kind::None, "{}"),
        (Kind::Bool, "{true}"),
        (Kind::I8, "{-8}"),
        (Kind::U8, "{8}"),
        (Kind::I16, "{-16}"),
        (Kind::U16, "{16}"),
        (Kind::I32, "{-32}"),
        (Kind::U32, "{32}"),
        (Kind::I64, "{-64}"),
        (Kind::U64, "{64}"),
        (Kind::F32, "{1.5}"),
        (Kind::Vector2, "{(1, 2)}"),
        (Kind::Vector3, "{(1, 2, 3)}"),
        (Kind::Vector4, "{(1, 2, 3, 4)}"),
        (
            Kind::Matrix44,
            "{(1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15, 4, 8, 12, 16)}",
        ),
        (Kind::Color, "{(1, 2, 3, 4)}"),
        (Kind::String, "{\"weapon\"}"),
        (Kind::Hash, "{1e6ba0c4}"),
        (Kind::WadChunkLink, "{00c9fd8f1a2b3c4d}"),
    ];
    let nodes = owned.nodes();
    for (kind, key) in expected {
        let trail = format!("{:08x}{key}", F_KEYS + kind as u32);
        assert!(nodes.contains(&(trail.clone(), C7)), "{kind:?}: {trail}");
    }
    assert_eq!(nodes.len(), 1 + expected.len());
}

#[test]
fn a_string_key_is_a_json_string() {
    let object = BinObject::builder(OBJECT, C1)
        .property(
            F_MAP_EMBED,
            values::Map::new(
                Kind::String,
                Kind::Struct,
                vec![(
                    values::String::from("a\"b\\c\n\u{1}").into(),
                    node(C7, vec![]).into(),
                )],
            )
            .unwrap(),
        )
        .build();
    let [owned, _] = record_both(&Bin::builder().object(object).build(), always_continue);
    assert_eq!(owned.nodes()[1].0, "0000000b{\"a\\\"b\\\\c\\n\\u0001\"}");
}

/// Records the trail's capacity at every node.
#[derive(Default)]
struct Capacities(Vec<(usize, usize)>);

impl<'a, V: TreeValue<'a>> Visitor<'a, V> for Capacities {
    type Error = Error;

    fn enter_node(&mut self, node: &Node<'_, 'a, V>) -> Result<Visit, Error> {
        let trail = node.trail();
        self.0
            .push((trail.steps.capacity(), trail.classes.capacity()));
        Ok(Visit::Continue)
    }
}

#[test]
fn a_map_of_ten_thousand_entries_grows_the_trail_once() {
    let entries: Vec<_> = (0..10_000u32)
        .map(|i| {
            (
                values::Hash::new(i).into(),
                PropertyValueEnum::Struct(node(C7, vec![])),
            )
        })
        .collect();
    let object = BinObject::builder(OBJECT, C1)
        .property(
            F_MAP_STRUCT,
            values::Map::new(Kind::Hash, Kind::Struct, entries).unwrap(),
        )
        .build();
    let bin = Bin::builder().object(object).build();

    let [(owned, _), (viewed, _)] = walk_both(&bin, Capacities::default);
    for visited in [owned, viewed] {
        assert_eq!(visited.0.len(), 10_001);
        // The root sees an empty trail; every entry after it sees the same two-step trail, at a
        // capacity that never moves once it is set.
        let first = visited.0[1];
        assert!(first.0 <= 4 && first.1 <= 4, "{first:?}");
        assert!(visited.0[1..].iter().all(|c| *c == first));
    }
}

const UIBASE: &[u8] = include_bytes!("../../tests/bins/lolminimap_uibase.bin");
const UIFLIPPED: &[u8] = include_bytes!("../../tests/bins/lolminimap_uiflipped.ptch.bin");

/// `(object, class, trail)` per node.
#[derive(Default)]
struct Roots(Vec<(u32, u32, String)>);

impl<'a, V: TreeValue<'a>> Visitor<'a, V> for Roots {
    type Error = Error;

    fn enter_node(&mut self, node: &Node<'_, 'a, V>) -> Result<Visit, Error> {
        self.0.push((
            node.object_hash().0,
            node.class_hash().0,
            node.trail().to_string(),
        ));
        Ok(Visit::Continue)
    }
}

#[test]
fn a_shipped_bin_walks_the_same_over_both_trees() {
    let bin = Bin::from_reader(&mut io::Cursor::new(UIBASE)).unwrap();
    let mut owned = Roots::default();
    bin.walk(&mut owned).unwrap();

    let mut stream = BinStream::mount(io::Cursor::new(UIBASE.to_vec())).unwrap();
    let mut viewed = Roots::default();
    stream.walk(&mut viewed).unwrap();

    assert_eq!(owned.0, viewed.0);
    let roots: Vec<_> = owned
        .0
        .iter()
        .filter(|(_, _, trail)| trail.is_empty())
        .map(|(object, _, _)| *object)
        .collect();
    assert_eq!(roots.len(), 66);
    assert_eq!(
        roots,
        bin.objects.keys().map(|h| h.0).collect::<Vec<_>>(),
        "roots in file order"
    );
    assert!(owned.0.len() > roots.len(), "the bin has nested nodes");
}

#[test]
fn an_override_walks_its_embedded_objects_and_never_a_record() {
    let patch = BinOverride::from_reader(&mut io::Cursor::new(UIFLIPPED)).unwrap();
    assert!(!patch.patches.is_empty());
    let mut visited = Roots::default();
    patch.walk(&mut visited).unwrap();

    let roots: Vec<_> = visited
        .0
        .iter()
        .filter(|(_, _, trail)| trail.is_empty())
        .map(|(object, _, _)| *object)
        .collect();
    assert_eq!(roots, patch.objects.keys().map(|h| h.0).collect::<Vec<_>>());
    for (object, _, _) in &visited.0 {
        assert!(patch.objects.contains_key(&BinHash(*object)));
    }
}

#[test]
fn a_mutable_reference_to_a_visitor_is_a_visitor() {
    let bin = fixture();
    let mut recorder = Recorder::new(always_continue);
    let by_ref: &mut dyn Visitor<'_, &PropertyValueEnum, Error = Error> = &mut recorder;
    let mut by_ref = by_ref;
    bin.walk(&mut by_ref).unwrap();
    assert_eq!(recorder.nodes().len(), EXPECTED_NODES.len());
}
