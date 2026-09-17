//! Tests for `ValuePath`, `MapKey` and their renderings, `value-walk.md` section 4.

use std::{
    borrow::Cow,
    collections::{hash_map::DefaultHasher, HashMap},
    hash::Hasher as _,
};

use glam::{Mat4, Vec2, Vec3, Vec4};
use ltk_hash::{BinHash, Hash as _, WadHash};
use ltk_primitives::Color;

use super::{FloatBits, MapKey, NamelessKind, ValuePath, ValueSegment};
use crate::path::FieldNames;
use crate::{
    property::{values, Kind},
    walk::Leaf,
    Error, PropertyValueEnum,
};

fn hash_of<T: std::hash::Hash>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn the_hash_form_writes_fields_as_hex_and_subscripts_in_brackets() {
    let mut path = ValuePath::new();
    path.push_field(BinHash(0x1e6b_a0c4), BinHash(0xC1A5_0001));
    path.push_index(3);
    path.push_field(BinHash(0x0000_00aa), BinHash(0xC1A5_0002));
    path.push_key(MapKey::String("weapon".into()));

    assert_eq!(path.to_string(), r#"1e6ba0c4[3].000000aa{"weapon"}"#);
    assert_eq!(ValuePath::new().to_string(), "");
}

#[test]
fn two_paths_with_the_same_segments_are_equal_whatever_their_classes() {
    let mut known = ValuePath::new();
    known.push_field(BinHash(1), BinHash(0xC1A5_0001));
    let unknown: ValuePath = [ValueSegment::Field(BinHash(1))].into_iter().collect();

    assert_eq!(known, unknown);
    assert_eq!(hash_of(&known), hash_of(&unknown));
    assert_eq!(
        known.fields().collect::<Vec<_>>(),
        [(BinHash(1), Some(BinHash(0xC1A5_0001)))]
    );
    assert_eq!(unknown.fields().collect::<Vec<_>>(), [(BinHash(1), None)]);
}

/// One value of every kind, and the key it converts to where a map can be keyed by it.
fn keys_and_values() -> Vec<(PropertyValueEnum, Option<MapKey>)> {
    let matrix = Mat4::from_cols_array(&[
        1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
    ]);
    let bits = |values: &[f32]| {
        values
            .iter()
            .copied()
            .map(FloatBits::new)
            .collect::<Vec<_>>()
    };
    vec![
        (values::None::default().into(), Some(MapKey::None)),
        (values::Bool::new(true).into(), Some(MapKey::Bool(true))),
        (values::I8::new(-8).into(), Some(MapKey::I8(-8))),
        (values::U8::new(8).into(), Some(MapKey::U8(8))),
        (values::I16::new(-16).into(), Some(MapKey::I16(-16))),
        (values::U16::new(16).into(), Some(MapKey::U16(16))),
        (values::I32::new(-32).into(), Some(MapKey::I32(-32))),
        (values::U32::new(32).into(), Some(MapKey::U32(32))),
        (values::I64::new(-64).into(), Some(MapKey::I64(-64))),
        (values::U64::new(64).into(), Some(MapKey::U64(64))),
        (
            values::F32::new(1.5).into(),
            Some(MapKey::F32(FloatBits::new(1.5))),
        ),
        (
            values::Vector2::new(Vec2::new(1.0, 2.0)).into(),
            Some(MapKey::Vector2(bits(&[1.0, 2.0]).try_into().unwrap())),
        ),
        (
            values::Vector3::new(Vec3::new(1.0, 2.0, 3.0)).into(),
            Some(MapKey::Vector3(bits(&[1.0, 2.0, 3.0]).try_into().unwrap())),
        ),
        (
            values::Vector4::new(Vec4::new(1.0, 2.0, 3.0, 4.0)).into(),
            Some(MapKey::Vector4(
                bits(&[1.0, 2.0, 3.0, 4.0]).try_into().unwrap(),
            )),
        ),
        (
            values::Matrix44::new(matrix).into(),
            Some(MapKey::Matrix44(
                bits(&matrix.transpose().to_cols_array())
                    .try_into()
                    .unwrap(),
            )),
        ),
        (
            values::Color::new(Color {
                r: 1u8,
                g: 2,
                b: 3,
                a: 4,
            })
            .into(),
            Some(MapKey::Color(Color {
                r: 1,
                g: 2,
                b: 3,
                a: 4,
            })),
        ),
        (
            values::String::from("weapon").into(),
            Some(MapKey::String("weapon".into())),
        ),
        (
            values::Hash::new(0x1e6b_a0c4u32).into(),
            Some(MapKey::Hash(BinHash(0x1e6b_a0c4))),
        ),
        (
            values::WadChunkLink::new(0x00c9_fd8f_1a2b_3c4du64).into(),
            Some(MapKey::File(WadHash(0x00c9_fd8f_1a2b_3c4d))),
        ),
        (values::ObjectLink::new(0x0bee_f000u32).into(), None),
        (values::BitBool::new(true).into(), None),
        (values::Struct::default().into(), None),
        (values::Embedded::default().into(), None),
        (values::Container::default().into(), None),
        (values::Optional::default().into(), None),
        (values::Map::default().into(), None),
    ]
}

#[test]
fn a_key_converts_from_every_kind_a_map_can_be_keyed_by_and_back() {
    for (value, expected) in keys_and_values() {
        let kind = value.kind();
        assert_eq!(kind.is_valid_map_key(), expected.is_some(), "{kind:?}");

        match (MapKey::try_from(&value), expected) {
            (Ok(key), Some(expected)) => {
                assert_eq!(key, expected, "{kind:?}");
                assert_eq!(key.kind(), kind);
                assert_eq!(key.to_value(), value, "{kind:?} round trip");
            }
            (Err(Error::InvalidKeyType(rejected)), None) => assert_eq!(rejected, kind),
            (got, expected) => panic!("{kind:?}: got {got:?}, expected {expected:?}"),
        }
    }
}

#[test]
fn a_key_converts_from_a_leaf_except_a_link_and_a_flag() {
    assert_eq!(
        MapKey::from_leaf(Leaf::String("weapon")),
        Some(MapKey::String("weapon".into()))
    );
    assert_eq!(
        MapKey::from_leaf(Leaf::F32(-0.0)),
        Some(MapKey::F32(FloatBits::new(-0.0)))
    );
    assert_eq!(MapKey::from_leaf(Leaf::Link(BinHash(1))), None);
    assert_eq!(MapKey::from_leaf(Leaf::Flag(true)), None);
}

#[test]
fn float_keys_are_equal_exactly_when_their_bits_are() {
    let nan = MapKey::F32(FloatBits::new(f32::NAN));
    assert_eq!(nan, MapKey::F32(FloatBits::new(f32::NAN)));
    assert_eq!(
        hash_of(&nan),
        hash_of(&MapKey::F32(FloatBits::new(f32::NAN)))
    );
    assert_ne!(
        MapKey::F32(FloatBits::new(0.0)),
        MapKey::F32(FloatBits::new(-0.0))
    );
    assert_eq!(Kind::F32, nan.kind());
}

const POSITION: &str = "Position";
const CLASS: BinHash = BinHash(0xC1A5_0001);

/// A path of one field, `Position` on `CLASS`, followed by `segment`.
fn position_then(segment: ValueSegment) -> ValuePath {
    let mut path = ValuePath::new();
    path.push_field(BinHash::hash_str(POSITION), CLASS);
    path.push(segment);
    path
}

fn names() -> HashMap<BinHash, String> {
    HashMap::from([
        (BinHash::hash_str(POSITION), POSITION.to_owned()),
        (BinHash::hash_str("Size"), "Size".to_owned()),
    ])
}

/// Names `Position` and the hash key `Weapon`.
struct Table;

impl FieldNames for Table {
    fn field(&self, field: BinHash, _class: Option<BinHash>) -> Option<Cow<'_, str>> {
        (field == BinHash::hash_str(POSITION)).then_some(Cow::Borrowed(POSITION))
    }

    fn hash(&self, hash: BinHash) -> Option<Cow<'_, str>> {
        (hash == BinHash::hash_str("Weapon")).then_some(Cow::Borrowed("Weapon"))
    }
}

/// Every row of `value-walk.md` section 4.2: the hash form, the named form, and the client path
/// or the kind that has none.
#[test]
fn every_segment_renders_in_all_three_forms() {
    let position = format!("{:08x}", BinHash::hash_str(POSITION));
    let bits = |v: &[f32]| v.iter().copied().map(FloatBits::new).collect::<Vec<_>>();
    // (segment after `Position`, hash form, named form, client path or the key kind with none)
    type Row = (Option<ValueSegment>, String, String, Result<String, Kind>);
    let rows: Vec<Row> = vec![
        (
            None,
            position.clone(),
            "Position".into(),
            Ok("Position".into()),
        ),
        (
            Some(ValueSegment::Index(3)),
            format!("{position}[3]"),
            "Position[3]".into(),
            Ok("Position[3]".into()),
        ),
        (
            Some(ValueSegment::Key(MapKey::I32(-12))),
            format!("{position}{{-12}}"),
            "Position{-12}".into(),
            Ok("Position{-12}".into()),
        ),
        (
            Some(ValueSegment::Key(MapKey::U64(12))),
            format!("{position}{{12}}"),
            "Position{12}".into(),
            Ok("Position{12}".into()),
        ),
        (
            Some(ValueSegment::Key(MapKey::Bool(true))),
            format!("{position}{{true}}"),
            "Position{true}".into(),
            Ok("Position{true}".into()),
        ),
        (
            Some(ValueSegment::Key(MapKey::F32(FloatBits::new(1.5)))),
            format!("{position}{{1.5}}"),
            "Position{1.5}".into(),
            Ok("Position{1.5}".into()),
        ),
        (
            Some(ValueSegment::Key(MapKey::String("weapon".into()))),
            format!("{position}{{\"weapon\"}}"),
            "Position{\"weapon\"}".into(),
            Ok("Position{\"weapon\"}".into()),
        ),
        (
            Some(ValueSegment::Key(MapKey::Hash(BinHash(0x1e6b_a0c4)))),
            format!("{position}{{1e6ba0c4}}"),
            "Position{1e6ba0c4}".into(),
            Ok("Position{510369988}".into()),
        ),
        (
            Some(ValueSegment::Key(MapKey::Hash(BinHash::hash_str("Weapon")))),
            format!("{position}{{{:08x}}}", BinHash::hash_str("Weapon")),
            "Position{\"Weapon\"}".into(),
            Ok(format!("Position{{{}}}", *BinHash::hash_str("Weapon"))),
        ),
        (
            Some(ValueSegment::Key(MapKey::File(WadHash(
                0x00c9_fd8f_1a2b_3c4d,
            )))),
            format!("{position}{{00c9fd8f1a2b3c4d}}"),
            "Position{00c9fd8f1a2b3c4d}".into(),
            Ok("Position{56855261380033613}".into()),
        ),
        (
            Some(ValueSegment::Key(MapKey::Vector2(
                bits(&[1.0, 2.0]).try_into().unwrap(),
            ))),
            format!("{position}{{(1, 2)}}"),
            "Position{(1, 2)}".into(),
            Err(Kind::Vector2),
        ),
        (
            Some(ValueSegment::Key(MapKey::Color(Color {
                r: 1,
                g: 2,
                b: 3,
                a: 4,
            }))),
            format!("{position}{{(1, 2, 3, 4)}}"),
            "Position{(1, 2, 3, 4)}".into(),
            Err(Kind::Color),
        ),
        (
            Some(ValueSegment::Key(MapKey::Matrix44(
                bits(&[
                    1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0,
                    15.0, 16.0,
                ])
                .try_into()
                .unwrap(),
            ))),
            format!("{position}{{(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)}}"),
            "Position{(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)}".into(),
            Err(Kind::Matrix44),
        ),
        (
            Some(ValueSegment::Key(MapKey::F32(FloatBits::new(f32::NAN)))),
            format!("{position}{{NaN}}"),
            "Position{NaN}".into(),
            Err(Kind::F32),
        ),
        (
            Some(ValueSegment::Key(MapKey::F32(FloatBits::new(
                f32::INFINITY,
            )))),
            format!("{position}{{inf}}"),
            "Position{inf}".into(),
            Err(Kind::F32),
        ),
        (
            Some(ValueSegment::Key(MapKey::None)),
            format!("{position}{{}}"),
            "Position{}".into(),
            Err(Kind::None),
        ),
    ];

    for (segment, hash_form, named_form, client) in rows {
        let mut path = position_then(ValueSegment::Index(0));
        path.pop();
        path.extend(segment);

        assert_eq!(path.to_string(), hash_form);
        assert_eq!(path.to_named(&Table).text, named_form, "{hash_form}");
        match (path.to_property_path(&Table), client) {
            (Ok(got), Ok(expected)) => assert_eq!(got.as_str(), expected),
            (Err(error), Err(kind)) => {
                assert_eq!(error.segment, 1);
                assert_eq!(error.kind, NamelessKind::Key(kind));
            }
            (got, expected) => panic!("{hash_form}: got {got:?}, expected {expected:?}"),
        }
    }
}

#[test]
fn a_float_key_with_no_json_literal_has_no_client_path() {
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let path = position_then(ValueSegment::Key(MapKey::F32(FloatBits::new(value))));
        assert_eq!(
            path.to_property_path(&Table).unwrap_err().kind,
            NamelessKind::Key(Kind::F32)
        );
    }
}

#[test]
fn the_first_nameless_segment_is_reported_not_the_last() {
    let unknown = BinHash(0x0bad_0001);
    let path: ValuePath = [
        ValueSegment::Field(BinHash::hash_str(POSITION)),
        ValueSegment::Field(unknown),
        ValueSegment::Key(MapKey::None),
        ValueSegment::Field(BinHash(0x0bad_0002)),
    ]
    .into_iter()
    .collect();

    let error = path.to_property_path(&Table).unwrap_err();
    assert_eq!(error.segment, 1);
    assert_eq!(
        error.kind,
        NamelessKind::Field {
            field: unknown,
            class: None
        }
    );
}

#[test]
fn a_name_that_does_not_hash_back_to_its_field_is_not_used() {
    let field = BinHash(0x0bad_0001);
    let table = HashMap::from([(field, "Position".to_owned())]);
    let path: ValuePath = [ValueSegment::Field(field)].into_iter().collect();

    assert!(path.to_property_path(&table).is_err());
    let named = path.to_named(&table);
    assert_eq!(named.text, "0bad0001");
    assert_eq!((named.named, named.unnamed), (0, 1));
}

#[test]
fn segments_that_spell_no_property_path_are_nameless() {
    let empty = ValuePath::new().to_property_path(&names()).unwrap_err();
    assert_eq!(empty.segment, 0);
    assert!(matches!(empty.kind, NamelessKind::Path(_)));

    let leading: ValuePath = [
        ValueSegment::Index(0),
        ValueSegment::Field(BinHash::hash_str("Size")),
    ]
    .into_iter()
    .collect();
    let error = leading.to_property_path(&names()).unwrap_err();
    assert_eq!(error.segment, 0);
    assert!(matches!(error.kind, NamelessKind::Path(_)));

    let mut twice = position_then(ValueSegment::Index(0));
    twice.push(ValueSegment::Index(1));
    let error = twice.to_property_path(&names()).unwrap_err();
    assert_eq!(error.segment, 2);
    assert!(matches!(error.kind, NamelessKind::Path(_)));
}

#[test]
fn the_named_form_counts_every_field_and_hash_key() {
    let path: ValuePath = [
        ValueSegment::Field(BinHash::hash_str(POSITION)),
        ValueSegment::Key(MapKey::Hash(BinHash::hash_str("Weapon"))),
        ValueSegment::Field(BinHash(0x0bad_0001)),
        ValueSegment::Key(MapKey::Hash(BinHash(0x0bad_0002))),
        ValueSegment::Index(4),
        ValueSegment::Key(MapKey::String("not counted".into())),
    ]
    .into_iter()
    .collect();

    let named = path.to_named(&Table);
    assert_eq!(
        named.text,
        r#"Position{"Weapon"}.0bad0001{0bad0002}[4]{"not counted"}"#
    );
    assert_eq!((named.named, named.unnamed), (2, 2));
    assert!(!named.is_complete());
    assert_eq!(named.to_string(), named.text);
    assert!(position_then(ValueSegment::Index(0))
        .to_named(&Table)
        .is_complete());
}

#[test]
fn every_name_table_shape_answers_by_its_key() {
    let size = BinHash::hash_str("Size");
    let path: ValuePath = [ValueSegment::Field(size)].into_iter().collect();
    let mut on_class = ValuePath::new();
    on_class.push_field(size, CLASS);

    assert!(path.to_property_path(&()).is_err());

    let by_field = names();
    assert_eq!(path.to_property_path(&by_field).unwrap().as_str(), "Size");
    assert_eq!(
        on_class.to_property_path(&&by_field).unwrap().as_str(),
        "Size"
    );

    let by_class = HashMap::from([((CLASS, size), "Size".to_owned())]);
    assert_eq!(
        on_class.to_property_path(&by_class).unwrap().as_str(),
        "Size"
    );
    assert_eq!(
        path.to_property_path(&by_class).unwrap_err().kind,
        NamelessKind::Field {
            field: size,
            class: None
        }
    );

    let dynamic: &dyn FieldNames = &by_class;
    assert_eq!(on_class.to_property_path(dynamic).unwrap().as_str(), "Size");
}

#[test]
fn popping_a_field_drops_its_class_and_popping_a_subscript_keeps_the_rest() {
    let mut path = ValuePath::new();
    path.push_field(BinHash(1), BinHash(0xC1A5_0001));
    path.push_index(0);
    path.push_field(BinHash(2), BinHash(0xC1A5_0002));

    assert_eq!(path.pop(), Some(ValueSegment::Field(BinHash(2))));
    assert_eq!(
        path.fields().collect::<Vec<_>>(),
        [(BinHash(1), Some(BinHash(0xC1A5_0001)))]
    );
    path.push_field(BinHash(3), BinHash(0));
    assert_eq!(
        path.fields().collect::<Vec<_>>(),
        [(BinHash(1), Some(BinHash(0xC1A5_0001))), (BinHash(3), None)]
    );

    assert_eq!(path.pop(), Some(ValueSegment::Field(BinHash(3))));
    assert_eq!(path.pop(), Some(ValueSegment::Index(0)));
    assert_eq!(path.pop(), Some(ValueSegment::Field(BinHash(1))));
    assert_eq!(path.pop(), None);
    assert_eq!(path.fields().count(), 0);
}

#[test]
fn an_index_past_u32_or_a_path_past_the_length_limit_is_no_property_path() {
    let size = BinHash::hash_str("Size");
    let far: ValuePath = [
        ValueSegment::Field(size),
        ValueSegment::Index(usize::try_from(u64::from(u32::MAX) + 1).unwrap()),
    ]
    .into_iter()
    .collect();
    let error = far.to_property_path(&names()).unwrap_err();
    assert_eq!(error.segment, 1);
    assert!(matches!(error.kind, NamelessKind::Path(_)));

    let name = "a".repeat(crate::path::PropertyPath::MAX_LEN + 1);
    let field = BinHash::hash_str(&name);
    let long: ValuePath = [ValueSegment::Field(field)].into_iter().collect();
    let error = long
        .to_property_path(&HashMap::from([(field, name)]))
        .unwrap_err();
    assert_eq!(error.segment, 0);
    assert!(matches!(error.kind, NamelessKind::Path(_)));
}

#[test]
fn a_key_and_a_segment_display_as_the_hash_form_writes_them() {
    let keys = [
        (MapKey::I32(-12), "-12"),
        (MapKey::F32(FloatBits::new(1.5)), "1.5"),
        (MapKey::String("a\"b".into()), r#""a\"b""#),
        (MapKey::Hash(BinHash(0x1e6b_a0c4)), "1e6ba0c4"),
        (
            MapKey::File(WadHash(0x00c9_fd8f_1a2b_3c4d)),
            "00c9fd8f1a2b3c4d",
        ),
        (
            MapKey::Color(Color {
                r: 1,
                g: 2,
                b: 3,
                a: 4,
            }),
            "(1, 2, 3, 4)",
        ),
        (MapKey::None, ""),
    ];
    for (key, text) in keys {
        assert_eq!(key.to_string(), text);
    }

    assert_eq!(ValueSegment::Field(BinHash(0xaa)).to_string(), "000000aa");
    assert_eq!(ValueSegment::Index(3).to_string(), "[3]");
    assert_eq!(
        ValueSegment::Key(MapKey::String("weapon".into())).to_string(),
        r#"{"weapon"}"#
    );
}
