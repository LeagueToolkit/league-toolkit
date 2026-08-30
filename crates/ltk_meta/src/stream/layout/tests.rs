use glam::{Mat4, Vec2, Vec3, Vec4};
use ltk_primitives::Color;

use crate::{
    concrete::values,
    property::values::{Embedded, UnorderedContainer},
    stream::layout::{Cursor, Numbering},
    traits::PropertyExt as _,
    Error, PropertyValueEnum,
};

/// A cursor over `bytes`, in the numbering everything shipped uses.
fn cursor(bytes: &[u8]) -> Cursor<'_> {
    Cursor::new(bytes, Numbering::Current)
}

/// The value's body bytes, exactly as the eager writer lays them out.
fn body(value: &PropertyValueEnum) -> Vec<u8> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    value.to_writer(&mut cursor).expect("the value writes");
    cursor.into_inner()
}

/// One constructed value per kind, complex kinds included.
fn one_of_each() -> Vec<PropertyValueEnum> {
    let object = values::Struct {
        class_hash: 0x1234u32.into(),
        properties: [
            (0x1111u32.into(), values::I32::new(42).into()),
            (0x2222u32.into(), values::String::from("hello").into()),
        ]
        .into_iter()
        .collect(),
        meta: Default::default(),
    };

    vec![
        values::None::default().into(),
        values::Bool::new(true).into(),
        values::BitBool::new(true).into(),
        values::I8::new(-1).into(),
        values::U8::new(1).into(),
        values::I16::new(-2).into(),
        values::U16::new(2).into(),
        values::I32::new(-3).into(),
        values::U32::new(3).into(),
        values::I64::new(-4).into(),
        values::U64::new(4).into(),
        values::F32::new(1.5).into(),
        values::Vector2::new(Vec2::ONE).into(),
        values::Vector3::new(Vec3::ONE).into(),
        values::Vector4::new(Vec4::ONE).into(),
        values::Matrix44::new(Mat4::IDENTITY).into(),
        values::Color::new(Color::new(1, 2, 3, 4)).into(),
        values::String::from("a string").into(),
        values::Hash::new(0xABCDu32).into(),
        values::WadChunkLink::new(0xDEAD_BEEFu64).into(),
        values::ObjectLink::new(0x4444u32).into(),
        object.clone().into(),
        // A null pointer: class hash 0, no size field, no body.
        values::Struct::default().into(),
        Embedded(object).into(),
        values::Container::from(vec![values::I32::new(1), values::I32::new(2)]).into(),
        values::Container::from(vec![
            values::String::from("one"),
            values::String::from("two"),
        ])
        .into(),
        UnorderedContainer(values::Container::from(vec![values::U8::new(9)])).into(),
        values::Optional::from(values::F32::new(2.5)).into(),
        values::Optional::empty(crate::PropertyKind::F32)
            .expect("F32 nests")
            .into(),
        values::Map::new(
            crate::PropertyKind::U32,
            crate::PropertyKind::String,
            vec![(
                values::U32::new(7).into(),
                values::String::from("seven").into(),
            )],
        )
        .expect("a valid map")
        .into(),
    ]
}

/// A skip's distance, a walk's distance, the bytes `take_value` hands back, the written bytes
/// and [`PropertyExt::size`] all have to agree, for every kind.
#[test]
fn skip_and_walk_distances_match_the_written_size() {
    for value in one_of_each() {
        let bytes = body(&value);
        assert_eq!(
            bytes.len(),
            value.size_no_header(),
            "{:?}: the writer and PropertyExt::size disagree",
            value.kind()
        );

        let mut cur = cursor(&bytes);
        cur.skip_value(value.kind()).expect("the value skips");
        assert_eq!(
            cur.position(),
            bytes.len(),
            "{:?}: skip distance is not the serialized size",
            value.kind()
        );

        let mut cur = cursor(&bytes);
        cur.walk_value(value.kind()).expect("the value walks");
        assert_eq!(
            cur.position(),
            bytes.len(),
            "{:?}: walk distance is not the serialized size",
            value.kind()
        );

        let mut cur = cursor(&bytes);
        assert_eq!(
            cur.take_value(value.kind()).expect("the value is taken"),
            bytes,
            "{:?}: the taken bytes are not the value's own",
            value.kind()
        );
        assert_eq!(cur.remaining(), 0);
    }
}

#[test]
fn shapes_come_from_the_header_bytes() {
    use crate::PropertyKind as K;
    for value in one_of_each() {
        let bytes = body(&value);
        let cur = cursor(&bytes);
        assert_eq!(
            cur.value_shape(value.kind()).expect("the shape reads"),
            crate::path::ValueShape::of(&value),
            "{:?}: the declared shape and ValueShape::of disagree",
            value.kind()
        );
        assert_eq!(cur.position(), 0, "reading a shape moved the cursor");
    }

    // Spot-check the interesting ones.
    let map: PropertyValueEnum = values::Map::empty(K::Hash, K::I32).expect("valid").into();
    let shape = cursor(&body(&map)).value_shape(K::Map).expect("reads");
    assert_eq!(shape.key_kind, Some(K::Hash));
    assert_eq!(shape.item_kind, Some(K::I32));
}

/// The layout core's leaf codecs and the `io::Read` codecs the primitive value types still carry
/// have to decode the same bytes the same way. Nothing on the parse path reaches the reader
/// codecs any more, so this is what keeps the two from drifting apart unnoticed.
#[test]
fn cursor_codecs_agree_with_the_reader_codecs() {
    use crate::{property::NoMeta, stream::owned, traits::ReadProperty};

    macro_rules! same_leaf {
        ($($owned:expr),* $(,)?) => {$({
            let owned = $owned;
            let value: PropertyValueEnum = owned.clone().into();
            let bytes = body(&value);

            let via_reader = ReadProperty::from_reader(
                &mut std::io::Cursor::new(&bytes[..]),
                false,
            )
            .expect("the reader codec reads");
            assert_eq!(owned, via_reader, "the reader codec disagrees with the writer");

            let via_cursor = owned::read_value::<NoMeta>(&mut cursor(&bytes), value.kind())
                .expect("the cursor codec reads");
            assert_eq!(value, via_cursor, "the two leaf codecs disagree");
        })*};
    }

    same_leaf!(
        values::Bool::new(true),
        values::BitBool::new(true),
        values::I8::new(-1),
        values::U8::new(200),
        values::I16::new(-300),
        values::U16::new(40_000),
        values::I32::new(-70_000),
        values::U32::new(3_000_000_000),
        values::I64::new(-5_000_000_000),
        values::U64::new(18_000_000_000_000_000_000),
        values::F32::new(-1.25),
        values::Vector2::new(Vec2::new(1.0, 2.0)),
        values::Vector3::new(Vec3::new(1.0, 2.0, 3.0)),
        values::Vector4::new(Vec4::new(1.0, 2.0, 3.0, 4.0)),
        values::Matrix44::new(Mat4::from_scale(Vec3::new(1.0, 2.0, 3.0))),
        values::Color::new(Color::new(1, 2, 3, 4)),
        values::Hash::new(0xABCDu32),
        values::WadChunkLink::new(0xDEAD_BEEF_0000_0001u64),
        values::ObjectLink::new(0x4444u32),
        values::String::from("héllo"),
    );
}

#[test]
fn an_object_walks_to_its_declared_size() {
    let object = crate::concrete::BinObject::builder(0x1111u32, 0x2222u32)
        .property(0x0001u32, values::I32::new(42))
        .property(0x0002u32, values::String::from("hello"))
        .build();

    let mut written = std::io::Cursor::new(Vec::new());
    object.to_writer(&mut written).expect("the object writes");
    let bytes = written.into_inner();

    let mut cur = cursor(&bytes);
    cur.walk_object().expect("the object walks");
    assert_eq!(cur.position(), bytes.len());

    // A size the property counts disagree with is the same error a value's would be.
    let mut lying = bytes.clone();
    let declared = u32::from_le_bytes(lying[..4].try_into().expect("4 bytes"));
    lying[..4].copy_from_slice(&(declared + 2).to_le_bytes());
    lying.extend_from_slice(&[0; 2]);

    let error = cursor(&lying)
        .walk_object()
        .expect_err("the counts disagree with the declared size");
    assert!(
        matches!(error, Error::InvalidSize(d, c) if d == u64::from(declared) + 2 && c == u64::from(declared)),
        "unexpected error: {error}"
    );
}

#[test]
fn a_lying_size_is_an_invalid_size_error() {
    let list: PropertyValueEnum =
        values::Container::from(vec![values::I32::new(1), values::I32::new(2)]).into();
    let mut bytes = body(&list);
    // The container's size field is bytes 1..5; the true body (count + two i32s) is 12.
    bytes[1..5].copy_from_slice(&20u32.to_le_bytes());
    bytes.extend_from_slice(&[0xFF; 8]); // the 8 padding bytes the lie claims

    let error = cursor(&bytes)
        .walk_value(crate::PropertyKind::Container)
        .expect_err("the counts disagree with the declared size");
    assert!(
        matches!(error, Error::InvalidSize(20, 12)),
        "unexpected error: {error}"
    );

    // The skip path is unaffected: the declared size is still the skip distance.
    let mut cur = cursor(&bytes);
    cur.skip_value(crate::PropertyKind::Container)
        .expect("skips");
    assert_eq!(cur.position(), bytes.len());
}

#[test]
fn leaf_codecs_decode_what_the_writer_wrote() {
    let value: PropertyValueEnum = values::String::from("héllo").into();
    let bytes = body(&value);
    let mut cur = cursor(&bytes);
    assert_eq!(cur.str_u16().expect("valid UTF-8"), "héllo");
    assert_eq!(cur.remaining(), 0);

    let mut cur = cursor(&bytes);
    assert!(matches!(cur.take(64), Err(Error::IOError(_))));
    assert_eq!(cur.position(), 0, "a failed take does not advance");

    // A length that cannot be a position at all is refused, not wrapped around.
    let mut cur = cursor(&bytes);
    assert!(matches!(cur.take(usize::MAX), Err(Error::IOError(_))));
}

/// A cursor carries its numbering, so the same bytes read two ways is one call.
#[test]
fn a_cursor_carries_the_numbering_its_bytes_use() {
    // Legacy numbering has no `WadChunkLink`, so `Struct` is 19 - which decodes as nothing at
    // all under the current numbering.
    let bytes = [19u8];

    let mut current = cursor(&bytes);
    assert_eq!(current.numbering(), Numbering::Current);
    assert!(matches!(
        current.kind(),
        Err(Error::InvalidPropertyTypePrimitive(_))
    ));

    let mut legacy = Cursor::new(&bytes, Numbering::Legacy);
    assert_eq!(
        legacy.kind().expect("19 is legacy Struct"),
        crate::PropertyKind::Struct
    );
}
