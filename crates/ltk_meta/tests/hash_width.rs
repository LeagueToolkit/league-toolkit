//! Tests the reading and the writing of `Hash` values of 4 and 8 bytes.

use std::io::{Cursor, Seek as _};

use ltk_hash::{BinHash, HashAlgorithm, HashValue, HashWidth, Hasher};
use ltk_meta::{
    path::{PropertyPath, ResolveErrorKind},
    property::{values, Kind},
    stream::{Numbering, ValueView},
    traits::{ReadProperty as _, WriteProperty as _},
    Bin, BinObject, BinOverride, BinStream, Error, PropertyValueEnum,
};
use proptest::prelude::*;

/// One `StaticMaterialDef` object of `Characters/Zac/Skins/Skin31.bin` from PBE 16.21. Its `name`
/// property is an 8-byte `Hash`.
const MATERIAL: &[u8] = include_bytes!("bins/zac_skin31_material.bin");
const MATERIAL_PATH: &str = "Characters/Zac/Skins/Skin31/Materials/ult";
const MATERIAL_NAME: u32 = 0x8d39_bde6;

const OBJECT: u32 = 0x0b1e_c700;
const CLASS: u32 = 0xc1a5_5000;

/// Property name hashes. The low byte of each hash is not a valid property kind byte. The walk
/// with 4-byte widths fails on that byte after an 8-byte `Hash`.
const A: u32 = 0x1000_00a1;
const B: u32 = 0x1000_00a2;
const C: u32 = 0x1000_00a3;

const WIDE: u64 = 0x5d07_ca0d_22ff_9588;

fn narrow(hash: u32) -> PropertyValueEnum {
    values::Hash::new(hash).into()
}

fn wide(hash: u64) -> PropertyValueEnum {
    values::Hash::new(HashValue::wide(hash)).into()
}

fn object(properties: impl IntoIterator<Item = (u32, PropertyValueEnum)>) -> Bin {
    let mut object = BinObject::new(OBJECT, CLASS);
    object.properties = properties
        .into_iter()
        .map(|(name, value)| (BinHash(name), value))
        .collect();
    Bin::builder().object(object).build()
}

fn embed(properties: impl IntoIterator<Item = (u32, PropertyValueEnum)>) -> values::Struct {
    values::Struct {
        class_hash: BinHash(CLASS),
        properties: properties
            .into_iter()
            .map(|(name, value)| (BinHash(name), value))
            .collect(),
    }
}

fn write(bin: &Bin) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    bin.to_writer(&mut out).expect("the bin writes");
    out.into_inner()
}

fn read(bytes: &[u8]) -> Result<Bin, Error> {
    Bin::from_reader(&mut Cursor::new(bytes))
}

/// Returns each property of the one object in `bytes`, decoded through its view.
fn viewed(bytes: &[u8]) -> Vec<(BinHash, PropertyValueEnum)> {
    let mut stream = BinStream::mount(Cursor::new(bytes)).expect("the bin mounts");
    let mut objects = stream.objects();
    let mut object = objects
        .next()
        .expect("the table reads")
        .expect("one object");
    let view = object.view().expect("the object views");
    view.properties()
        .map(|property| {
            let property = property.expect("the property reads");
            (
                property.name_hash(),
                property.value().expect("the value decodes"),
            )
        })
        .collect()
}

/// Writes `bin`, reads the bytes with the eager reader and with the views, and writes the result
/// again. Asserts that both readers return the values of `bin` and that the second write equals
/// the first write. Returns the bytes.
fn assert_round_trip(bin: &Bin) -> Vec<u8> {
    let bytes = write(bin);
    let back = read(&bytes).expect("the bin reads");
    assert_eq!(&back, bin, "the eager reader returns the written values");
    assert_eq!(
        write(&back),
        bytes,
        "the second write equals the first write"
    );

    let object = bin.objects.values().next().expect("one object");
    let owned: Vec<_> = object
        .properties
        .iter()
        .map(|(name, value)| (*name, value.clone()))
        .collect();
    assert_eq!(viewed(&bytes), owned, "the views return the written values");
    bytes
}

/// Calls `check` with the value view of property `name` of the one object in `bytes`.
fn with_view(bytes: &[u8], name: u32, check: impl FnOnce(ValueView<'_>)) {
    let mut stream = BinStream::mount(Cursor::new(bytes)).expect("the bin mounts");
    let mut objects = stream.objects();
    let mut object = objects
        .next()
        .expect("the table reads")
        .expect("one object");
    let view = object.view().expect("the object views");
    let property = view
        .property(name)
        .expect("the properties read")
        .expect("the property is present");
    check(property.value_view().expect("the value views"));
}

#[test]
fn bin_round_trips_shipped_wide_hash() {
    let bin = read(MATERIAL).expect("the material reads");
    let material = bin.objects.values().next().expect("one object");

    let hasher = Hasher {
        width: HashWidth::W8,
        algorithm: HashAlgorithm::Xxh3,
        lowercased: true,
    };
    assert_eq!(
        material.properties.get(&BinHash(MATERIAL_NAME)),
        Some(&values::Hash::new(hasher.hash_str(MATERIAL_PATH)).into())
    );
    assert_eq!(write(&bin), MATERIAL);
}

#[test]
fn numbering_stays_current_for_wide_hash() {
    let mut stream = BinStream::mount(Cursor::new(MATERIAL)).expect("the material mounts");
    {
        let mut objects = stream.objects();
        let mut object = objects
            .next()
            .expect("the table reads")
            .expect("one object");
        let view = object.view().expect("the object views");

        let name = view
            .property(MATERIAL_NAME)
            .expect("the properties read")
            .expect("the name is present");
        assert_eq!(name.raw().len(), 8);
        let ValueView::Hash(name) = name.value_view().expect("the name views") else {
            panic!("the name is a hash");
        };
        assert_eq!(name.width(), HashWidth::W8);
    }
    assert_eq!(stream.numbering(), Numbering::Current);

    let eager = read(MATERIAL).expect("the material reads");
    let object = eager.objects.values().next().expect("one object");
    let owned: Vec<_> = object
        .properties
        .iter()
        .map(|(name, value)| (*name, value.clone()))
        .collect();
    assert_eq!(viewed(MATERIAL), owned);
}

#[test]
fn bin_round_trips_wide_hash_before_other_property() {
    assert_round_trip(&object([
        (A, wide(WIDE)),
        (B, values::U32::new(7).into()),
        (C, values::String::from("after").into()),
    ]));
}

#[test]
fn bin_round_trips_wide_hash_as_last_property() {
    assert_round_trip(&object([(A, values::U32::new(7).into()), (B, wide(WIDE))]));
}

/// An 8-byte value below 2^32 is stored as 8 bytes. The upper 4 bytes are zero.
#[test]
fn bin_round_trips_wide_hash_of_zero() {
    let bytes = assert_round_trip(&object([(A, wide(0)), (B, narrow(0))]));
    with_view(&bytes, A, |value| {
        assert!(matches!(value, ValueView::Hash(hash) if hash == HashValue::wide(0)));
    });
    with_view(&bytes, B, |value| {
        assert!(matches!(value, ValueView::Hash(hash) if hash == HashValue::narrow(0)));
    });
}

#[test]
fn bin_round_trips_both_widths_in_one_object() {
    assert_round_trip(&object([
        (A, narrow(0xcafe_babe)),
        (B, wide(WIDE)),
        (C, narrow(1)),
    ]));
    assert_round_trip(&object([
        (A, wide(WIDE)),
        (B, narrow(0xcafe_babe)),
        (C, wide(1)),
    ]));
    assert_round_trip(&object([(A, wide(1)), (B, wide(2)), (C, wide(3))]));
}

#[test]
fn bin_round_trips_wide_hash_in_embedded() {
    assert_round_trip(&object([
        (
            A,
            values::Embedded(embed([
                (A, wide(WIDE)),
                (B, values::Bool::new(true).into()),
            ]))
            .into(),
        ),
        (B, narrow(5)),
    ]));
}

#[test]
fn bin_round_trips_wide_hash_in_struct_in_container() {
    let item = |hash| embed([(A, values::F32::new(1.5).into()), (B, wide(hash))]);
    assert_round_trip(&object([
        (A, values::Container::from(vec![item(1), item(WIDE)]).into()),
        (B, wide(WIDE)),
    ]));
}

#[test]
fn container_view_get_reads_wide_hash_items() {
    let items = [3, WIDE, 0].map(|hash| values::Hash::new(HashValue::wide(hash)));
    let bytes = assert_round_trip(&object([
        (A, values::Container::from(items.to_vec()).into()),
        (B, values::U8::new(1).into()),
    ]));

    with_view(&bytes, A, |value| {
        let ValueView::Container(list) = value else {
            panic!("the property is a container");
        };
        assert_eq!(list.len(), 3);
        for (index, item) in items.iter().enumerate() {
            let got = list.get(index as u32).expect("the item reads");
            assert!(matches!(got, Some(ValueView::Hash(hash)) if hash == item.value));
        }
        assert_eq!(list.iter().count(), 3);
    });
}

#[test]
fn bin_round_trips_map_with_wide_hash_keys() {
    let key = |hash| wide(hash);
    let fixed = values::Map::new(
        Kind::Hash,
        Kind::U32,
        vec![
            (key(1), values::U32::new(10).into()),
            (key(WIDE), values::U32::new(20).into()),
        ],
    )
    .expect("a valid map");
    let structs = values::Map::new(
        Kind::Hash,
        Kind::Embedded,
        vec![
            (key(0), values::Embedded(embed([(A, narrow(1))])).into()),
            (key(WIDE), values::Embedded(embed([(A, wide(2))])).into()),
        ],
    )
    .expect("a valid map");
    let strings = values::Map::new(
        Kind::Hash,
        Kind::String,
        vec![
            (key(WIDE), values::String::from("one").into()),
            (key(2), values::String::from("").into()),
        ],
    )
    .expect("a valid map");

    let bytes = assert_round_trip(&object([
        (A, fixed.into()),
        (B, structs.into()),
        (C, strings.into()),
    ]));

    with_view(&bytes, A, |value| {
        let ValueView::Map(map) = value else {
            panic!("the property is a map");
        };
        let keys: Vec<_> = map
            .iter()
            .map(|entry| match entry.expect("the entry reads") {
                (ValueView::Hash(key), ValueView::U32(_)) => key,
                _ => panic!("the entry is a hash and a u32"),
            })
            .collect();
        assert_eq!(keys, [HashValue::wide(1), HashValue::wide(WIDE)]);
    });
}

#[test]
fn bin_round_trips_map_with_wide_hash_values() {
    let narrow_keys = values::Map::new(
        Kind::Hash,
        Kind::Hash,
        vec![(narrow(1), narrow(2)), (narrow(3), narrow(4))],
    )
    .expect("a valid map");
    let wide_sides = values::Map::new(
        Kind::Hash,
        Kind::Hash,
        vec![(wide(1), wide(2)), (wide(3), wide(WIDE))],
    )
    .expect("a valid map");
    let wide_values = values::Map::new(
        Kind::String,
        Kind::Hash,
        vec![(values::String::from("k").into(), wide(WIDE))],
    )
    .expect("a valid map");

    assert_round_trip(&object([
        (A, narrow_keys.into()),
        (B, wide_sides.into()),
        (C, wide_values.into()),
    ]));
}

/// A map with 4-byte `Hash` keys and 8-byte `Hash` values has 12 bytes per entry. A map with
/// 8-byte keys and 4-byte values has the same size.
#[test]
fn from_reader_fails_on_map_with_ambiguous_hash_widths() {
    let map =
        values::Map::new(Kind::Hash, Kind::Hash, vec![(narrow(1), wide(2))]).expect("a valid map");
    let bytes = write(&object([(A, map.into())]));

    assert!(matches!(read(&bytes), Err(Error::AmbiguousHashWidth)));
}

#[test]
fn bin_round_trips_wide_hash_in_optional() {
    let some = |hash| values::Optional::from(values::Hash::new(HashValue::wide(hash)));
    let bytes = assert_round_trip(&object([
        (A, some(WIDE).into()),
        (B, values::U32::new(7).into()),
        (
            C,
            values::Optional::empty(Kind::Hash)
                .expect("a hash nests")
                .into(),
        ),
    ]));
    assert_round_trip(&object([
        (A, values::U32::new(7).into()),
        (B, some(0).into()),
    ]));

    with_view(&bytes, A, |value| {
        let ValueView::Optional(option) = value else {
            panic!("the property is an optional");
        };
        let got = option.get().expect("the item reads");
        assert!(matches!(got, Some(ValueView::Hash(hash)) if hash == HashValue::wide(WIDE)));
    });
}

/// The walk with 4-byte widths succeeds on this object. It reads the upper 4 bytes of the hash as
/// a property name. It reads the low byte of the next name as the kind byte of a `u64`. The
/// reader returns that result. The written bytes equal the input bytes.
#[test]
fn from_reader_reads_wide_hash_as_narrow_if_next_name_byte_is_kind() {
    const U64_KIND: u32 = 0x1000_0009;
    let bin = object([(A, wide(WIDE)), (U64_KIND, values::U32::new(7).into())]);
    let bytes = write(&bin);

    let back = read(&bytes).expect("the bin reads");
    let object = back.objects.values().next().expect("one object");
    assert_eq!(
        object.properties.get(&BinHash(A)),
        Some(&narrow(WIDE as u32))
    );
    assert_eq!(write(&back), bytes);
}

#[test]
fn from_reader_fails_with_invalid_size_if_no_width_fits() {
    let mut bytes = write(&object([(A, narrow(1)), (B, values::U16::new(2).into())]));
    // The object and its declared size are 1 byte longer. No assignment of widths ends at the
    // declared size.
    let size_at = bytes.len() - (4 + 4 + 2 + 5 + 4 + 5 + 2);
    bytes.push(0);
    let size = u32::from_le_bytes(bytes[size_at..size_at + 4].try_into().unwrap()) + 1;
    bytes[size_at..size_at + 4].copy_from_slice(&size.to_le_bytes());

    assert!(matches!(read(&bytes), Err(Error::InvalidSize(_, _))));
}

#[test]
fn container_and_map_fail_on_second_hash_width() {
    let mismatch = |result: Result<(), Error>| {
        assert!(matches!(
            result,
            Err(Error::MismatchedHashWidths {
                expected: HashWidth::W4,
                got: HashWidth::W8,
            })
        ));
    };

    mismatch(values::Container::new(Kind::Hash, vec![narrow(1), wide(2)]).map(|_| ()));
    let mut list = values::Container::new(Kind::Hash, vec![narrow(1)]).expect("a valid list");
    mismatch(list.push(wide(2)));
    assert_eq!(list.hash_width(), Some(HashWidth::W4));

    let value = || values::U8::new(0).into();
    mismatch(
        values::Map::new(
            Kind::Hash,
            Kind::U8,
            vec![(narrow(1), value()), (wide(2), value())],
        )
        .map(|_| ()),
    );
    let mut map =
        values::Map::new(Kind::U8, Kind::Hash, vec![(value(), narrow(1))]).expect("a valid map");
    mismatch(map.push(value(), wide(2)));
    assert_eq!(map.value_hash_width(), Some(HashWidth::W4));
    assert_eq!(map.key_hash_width(), None);
}

/// `Container::from` does not check widths. `to_writer` fails on a container with two widths.
#[test]
fn container_to_writer_fails_on_two_hash_widths() {
    let list = values::Container::from(vec![
        values::Hash::new(1u32),
        values::Hash::new(HashValue::wide(2)),
    ]);
    let error = list
        .to_writer(&mut Cursor::new(Vec::new()), false)
        .expect_err("the container holds two widths");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
}

#[test]
fn bin_override_round_trips_wide_hash_record() {
    let path = |text| PropertyPath::new(text).expect("a valid path");
    let patch = BinOverride::builder()
        .set(
            0x0001_u32,
            path("Name"),
            values::Hash::new(HashValue::wide(WIDE)),
        )
        .set(0x0002_u32, path("Name"), values::Hash::new(0xcafe_babe_u32))
        .set(
            0x0003_u32,
            path("Maybe"),
            values::Optional::from(values::Hash::new(HashValue::wide(1))),
        )
        .set(
            0x0004_u32,
            path("Maybe"),
            values::Optional::from(values::U64::new(1)),
        )
        .set(
            0x0005_u32,
            path("Node"),
            values::Embedded(embed([(A, wide(WIDE)), (B, narrow(1))])),
        )
        .build();

    let mut out = Cursor::new(Vec::new());
    patch.to_writer(&mut out).expect("the patch writes");
    let bytes = out.into_inner();

    let back = BinOverride::from_reader(&mut Cursor::new(&bytes)).expect("the patch reads");
    assert_eq!(back, patch);

    let mut out = Cursor::new(Vec::new());
    back.to_writer(&mut out).expect("the patch writes");
    assert_eq!(out.into_inner(), bytes);
}

/// `PropertyValueEnum::from_reader` resolves the widths inside a self-sized value. It leaves the
/// reader immediately after the value.
#[test]
fn property_value_from_reader_resolves_wide_hash() {
    let value: PropertyValueEnum = values::Embedded(embed([
        (A, wide(WIDE)),
        (B, values::String::from("x").into()),
    ]))
    .into();
    let mut bytes = Cursor::new(Vec::new());
    value.to_writer(&mut bytes).expect("the value writes");
    let len = bytes.position();
    bytes.get_mut().extend([0xff; 600]);
    bytes.rewind().unwrap();

    let back =
        PropertyValueEnum::from_reader(&mut bytes, Kind::Embedded, false).expect("the value reads");
    assert_eq!(back, value);
    assert_eq!(bytes.position(), len);
}

/// No declared size contains a `Hash` that is read on its own. `Hash::from_reader` reads 4
/// bytes.
#[test]
fn hash_from_reader_with_width_reads_given_width() {
    let bytes = WIDE.to_le_bytes();

    let narrow = values::Hash::from_reader(&mut Cursor::new(bytes), false).expect("4 bytes read");
    assert_eq!(narrow.value, HashValue::narrow(WIDE as u32));

    let wide = values::Hash::from_reader_with_width(&mut Cursor::new(bytes), HashWidth::W8)
        .expect("8 bytes read");
    assert_eq!(wide.value, HashValue::wide(WIDE));
}

#[test]
fn resolve_selects_wide_hash_key_by_number_only() {
    let map = values::Map::new(
        Kind::Hash,
        Kind::U32,
        vec![(wide(WIDE), values::U32::new(20).into())],
    )
    .expect("a valid map");
    let mut object = BinObject::new(OBJECT, CLASS);
    object
        .properties
        .insert(BinHash::from("Lookup"), map.into());

    let path = |text: &str| PropertyPath::new(text).expect("a valid path");
    assert_eq!(
        object.resolve(&path(&format!("Lookup{{{WIDE}}}"))).ok(),
        Some(&values::U32::new(20).into())
    );
    assert_eq!(
        object
            .resolve(&path(r#"Lookup{"weapon"}"#))
            .expect_err("a string has no 8-byte hash")
            .kind(),
        ResolveErrorKind::InvalidKey(Kind::Hash)
    );
}

#[test]
fn hash_serializes_wide_value_as_hex_string() {
    let narrow = values::Hash::new(0xcafe_babe_u32);
    let wide = values::Hash::new(HashValue::wide(0xcafe_babe));

    let narrow_json = serde_json::to_string(&narrow).unwrap();
    let wide_json = serde_json::to_string(&wide).unwrap();
    assert_eq!(narrow_json, r#"{"value":3405691582}"#);
    assert_eq!(wide_json, r#"{"value":"0x00000000cafebabe"}"#);

    assert_eq!(
        serde_json::from_str::<values::Hash>(&narrow_json).unwrap(),
        narrow
    );
    assert_eq!(
        serde_json::from_str::<values::Hash>(&wide_json).unwrap(),
        wide
    );
    assert!(serde_json::from_str::<values::Hash>(r#"{"value":4294967296}"#).is_err());
    assert!(serde_json::from_str::<values::Hash>(r#"{"value":"0xcafebabe"}"#).is_err());
}

/// The search selects 4-byte keys for a map if the entries end at the declared size with them.
/// The first walk selects the same widths. The result does not depend on the other properties of
/// the object.
#[test]
fn from_reader_reads_wide_map_keys_as_narrow_if_narrow_keys_fit() {
    let map = |key: PropertyValueEnum, value: &str| -> PropertyValueEnum {
        values::Map::new(
            Kind::Hash,
            Kind::String,
            vec![(key, values::String::from(value).into())],
        )
        .expect("a valid map")
        .into()
    };
    // The upper 4 bytes of the key are valid as the length of a string of 6 bytes.
    let written = map(wide(0x0000_0006_1234_5678), "ab");
    let expected = map(narrow(0x1234_5678), "\0\0\u{2}\0ab");

    // The first walk succeeds on this object.
    let bytes = write(&object([(A, written.clone())]));
    let back = read(&bytes).expect("the bin reads");
    let properties = &back.objects.values().next().expect("one object").properties;
    assert_eq!(properties.get(&BinHash(A)), Some(&expected));
    assert_eq!(write(&back), bytes);

    // The first walk fails on the 8-byte `Hash` of this object. The search runs.
    let bytes = write(&object([(A, wide(WIDE)), (B, written)]));
    let back = read(&bytes).expect("the bin reads");
    let properties = &back.objects.values().next().expect("one object").properties;
    assert_eq!(properties.get(&BinHash(A)), Some(&wide(WIDE)));
    assert_eq!(properties.get(&BinHash(B)), Some(&expected));
    assert_eq!(write(&back), bytes);
}

/// The first walk fails on this object. The search reads the hash as 4 bytes. It reads the upper
/// 4 bytes of the hash as a property name. It reads the low byte of the next name as the kind
/// byte of a `Hash`. It reads that `Hash` as 8 bytes. The written bytes equal the input bytes.
#[test]
fn from_reader_reads_wide_hash_as_narrow_if_next_name_byte_is_hash_kind() {
    const HASH_KIND: u32 = 0x1000_0011;
    let bin = object([(A, wide(WIDE)), (HASH_KIND, values::U32::new(7).into())]);
    let bytes = write(&bin);

    let back = read(&bytes).expect("the bin reads");
    let properties = &back.objects.values().next().expect("one object").properties;
    assert_eq!(properties.len(), 2);
    assert_eq!(properties.get(&BinHash(A)), Some(&narrow(WIDE as u32)));
    assert!(matches!(
        properties.get(&BinHash((WIDE >> 32) as u32)),
        Some(PropertyValueEnum::Hash(hash)) if hash.width() == HashWidth::W8
    ));
    assert_eq!(write(&back), bytes);
}

/// Returns the bytes of one object for `BinObject::from_reader`.
///
/// The object starts with a run of `0x11` bytes. Each offset in the run is a valid `Hash`
/// property. A container of `count` 8-byte hashes follows the run. One byte follows the
/// container. No assignment of widths ends at the declared size of the object.
fn object_with_repeated_wide_container(count: usize) -> Vec<u8> {
    const HASH_PROPERTIES: usize = 24;
    const WIDE_HASH_PROPERTIES: usize = 12;

    let mut body = Vec::new();
    body.extend(OBJECT.to_le_bytes());
    body.extend((HASH_PROPERTIES as u16 + 1).to_le_bytes());
    body.resize(
        body.len() + 9 * HASH_PROPERTIES + 4 * WIDE_HASH_PROPERTIES,
        0x11,
    );
    body.extend(A.to_le_bytes());
    body.push(0x80);
    body.push(0x11);
    body.extend((4 + 8 * count as u32).to_le_bytes());
    body.extend((count as u32).to_le_bytes());
    body.resize(body.len() + 8 * count, 0xff);
    body.push(0xff);

    let mut bytes = (body.len() as u32).to_le_bytes().to_vec();
    bytes.extend(body);
    bytes
}

/// The search walks the container once for each assignment of the `Hash` properties before it.
/// The step limit includes the items of the container. Without that part of the limit, this
/// input takes more than 40 seconds in a release build.
#[test]
fn bin_object_from_reader_fails_at_step_limit_in_bounded_time() {
    let bytes = object_with_repeated_wide_container(1 << 18);

    let start = std::time::Instant::now();
    let result = BinObject::from_reader(&mut Cursor::new(&bytes), BinHash(CLASS), false);

    assert!(result.is_err());
    assert!(start.elapsed() < std::time::Duration::from_secs(10));
}

/// A reader that counts the bytes that it returns.
struct Counting {
    inner: Cursor<Vec<u8>>,
    read: usize,
}

impl Counting {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            inner: Cursor::new(bytes),
            read: 0,
        }
    }
}

impl std::io::Read for Counting {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read = self.inner.read(buf)?;
        self.read += read;
        Ok(read)
    }
}

impl std::io::Seek for Counting {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(pos)
    }
}

/// The header of this container declares a container as its item kind and a size of 2 GiB. The
/// reader returns the error of the header after the first read.
#[test]
fn property_value_from_reader_returns_header_error_without_more_reads() {
    let mut bytes = vec![0x80];
    bytes.extend(0x7fff_ffff_u32.to_le_bytes());
    bytes.resize(1 << 20, 0);
    let mut reader = Counting::new(bytes);

    let result = PropertyValueEnum::from_reader(&mut reader, Kind::Container, false);

    assert!(matches!(result, Err(Error::InvalidNesting(_))));
    assert!(reader.read <= 4096, "the reader read {} bytes", reader.read);
}

/// The walk with 4-byte widths reads the low byte of the next name as the kind byte of a string.
/// It reads the next 2 bytes of the name as a length of 65,535 bytes. It fails at the end of the
/// embed. The first read contains the whole embed. The search runs without more reads.
#[test]
fn property_value_from_reader_resolves_wide_hash_without_more_reads() {
    const STRING_KIND: u32 = 0x10ff_ff10;
    let value: PropertyValueEnum = values::Embedded(embed([
        (A, wide(WIDE)),
        (STRING_KIND, values::U32::new(7).into()),
    ]))
    .into();
    let mut bytes = Cursor::new(Vec::new());
    value.to_writer(&mut bytes).expect("the value writes");
    let len = bytes.position();
    let mut bytes = bytes.into_inner();
    bytes.resize(1 << 20, 0);
    let mut reader = Counting::new(bytes);

    let back = PropertyValueEnum::from_reader(&mut reader, Kind::Embedded, false)
        .expect("the value reads");

    assert_eq!(back, value);
    assert_eq!(reader.inner.position(), len);
    assert!(reader.read <= 4096, "the reader read {} bytes", reader.read);
}

#[test]
fn merge_replaces_map_if_hash_key_widths_differ() {
    let map = |key: PropertyValueEnum, value: u32| -> PropertyValueEnum {
        values::Map::new(
            Kind::Hash,
            Kind::U32,
            vec![(key, values::U32::new(value).into())],
        )
        .expect("a valid map")
        .into()
    };
    let mut base = map(wide(1), 10);
    let edited = map(narrow(2), 20);

    let report = base.merge(&edited);

    assert_eq!(base, edited);
    assert_eq!(report.keys_inserted, 0);
    assert_eq!(report.replaced.len(), 1);
    assert!(report.replaced[0].mismatched);
}

#[test]
fn merge_replaces_map_if_hash_value_widths_differ() {
    let map = |entries: &[(u32, PropertyValueEnum)]| -> PropertyValueEnum {
        let entries = entries
            .iter()
            .map(|(key, value)| (values::U32::new(*key).into(), value.clone()))
            .collect();
        values::Map::new(Kind::U32, Kind::Hash, entries)
            .expect("a valid map")
            .into()
    };
    let mut base = map(&[(1, wide(1)), (2, wide(2))]);
    let edited = map(&[(2, narrow(7))]);

    let report = base.merge(&edited);

    assert_eq!(base, edited);
    assert_eq!(report.replaced.len(), 1);
    assert!(report.replaced[0].mismatched);
    base.to_writer(&mut Cursor::new(Vec::new()))
        .expect("the merged map writes");
}

#[test]
fn merge_combines_maps_with_equal_hash_key_widths() {
    let map = |entries: &[(u64, u32)]| -> PropertyValueEnum {
        let entries = entries
            .iter()
            .map(|(key, value)| (wide(*key), values::U32::new(*value).into()))
            .collect();
        values::Map::new(Kind::Hash, Kind::U32, entries)
            .expect("a valid map")
            .into()
    };
    let mut base = map(&[(1, 10)]);

    let report = base.merge(&map(&[(1, 11), (2, 20)]));

    assert_eq!(base, map(&[(1, 11), (2, 20)]));
    assert_eq!(report.keys_inserted, 1);
    assert_eq!(report.replaced.len(), 1);
    assert!(!report.replaced[0].mismatched);
}

#[test]
fn merge_reports_mismatched_if_hash_widths_differ() {
    let mut base = wide(1);

    let report = base.merge(&narrow(1));

    assert_eq!(base, narrow(1));
    assert_eq!(report.replaced.len(), 1);
    assert!(report.replaced[0].mismatched);
}

/// `Hash::new` hashes a string with FNV-1a 32 and stores the hash in 4 bytes.
#[test]
fn hash_new_hashes_str_at_width_4() {
    let hash = values::Hash::new("weapon");

    assert_eq!(hash.value, HashValue::from(BinHash::from("weapon")));
    assert_eq!(hash.value.width(), HashWidth::W4);
}

fn any_hash(width: HashWidth) -> BoxedStrategy<values::Hash> {
    match width {
        HashWidth::W4 => any::<u32>().prop_map(values::Hash::new).boxed(),
        HashWidth::W8 => prop_oneof![any::<u64>(), 0u64..3]
            .prop_map(|hash| values::Hash::new(HashValue::wide(hash)))
            .boxed(),
    }
}

fn any_width() -> impl Strategy<Value = HashWidth> {
    prop::sample::select(vec![HashWidth::W4, HashWidth::W8])
}

fn any_leaf() -> BoxedStrategy<PropertyValueEnum> {
    prop_oneof![
        any_width().prop_flat_map(any_hash).prop_map(Into::into),
        any::<u32>().prop_map(|v| values::U32::new(v).into()),
        any::<u64>().prop_map(|v| values::U64::new(v).into()),
        any::<bool>().prop_map(|v| values::Bool::new(v).into()),
        "[a-z]{0,6}".prop_map(|v| values::String::from(v).into()),
        any::<u32>().prop_map(|v| values::ObjectLink::new(v).into()),
    ]
    .boxed()
}

/// Returns a strategy for a property value. Structs nest at most `depth` levels.
fn any_value(depth: u32) -> BoxedStrategy<PropertyValueEnum> {
    let hashes = |width| prop::collection::vec(any_hash(width), 0..4);
    let mut options = vec![
        any_leaf(),
        any_width()
            .prop_flat_map(hashes)
            .prop_map(|items| values::Container::from(items).into())
            .boxed(),
        any_width()
            .prop_flat_map(any_hash)
            .prop_map(|hash| values::Optional::from(hash).into())
            .boxed(),
        any_width()
            .prop_flat_map(|width| prop::collection::vec((any_hash(width), any::<u32>()), 0..4))
            .prop_map(|entries| {
                let entries = entries
                    .into_iter()
                    .map(|(key, value)| (key.into(), values::U32::new(value).into()))
                    .collect();
                values::Map::new(Kind::Hash, Kind::U32, entries)
                    .expect("a valid map")
                    .into()
            })
            .boxed(),
    ];

    if depth > 0 {
        let node = || any_properties(depth - 1).prop_map(embed);
        options.push(
            node()
                .prop_map(|node| values::Embedded(node).into())
                .boxed(),
        );
        options.push(node().prop_map(Into::into).boxed());
        options.push(
            prop::collection::vec(node().prop_map(values::Embedded), 0..3)
                .prop_map(|items| values::Container::from(items).into())
                .boxed(),
        );
        options.push(
            any_width()
                .prop_flat_map(move |width| {
                    prop::collection::vec((any_hash(width), any_properties(depth - 1)), 0..3)
                })
                .prop_map(|entries| {
                    let entries = entries
                        .into_iter()
                        .map(|(key, node)| (key.into(), values::Embedded(embed(node)).into()))
                        .collect();
                    values::Map::new(Kind::Hash, Kind::Embedded, entries)
                        .expect("a valid map")
                        .into()
                })
                .boxed(),
        );
    }
    prop::strategy::Union::new(options).boxed()
}

/// Returns a strategy for properties. The low byte of each property name is not a valid kind
/// byte.
fn any_properties(depth: u32) -> BoxedStrategy<Vec<(u32, PropertyValueEnum)>> {
    prop::collection::vec(any_value(depth), 0..6)
        .prop_map(|values| {
            values
                .into_iter()
                .enumerate()
                .map(|(index, value)| (0x1000_00a0 + index as u32, value))
                .collect()
        })
        .boxed()
}

proptest! {
    #[test]
    fn bin_round_trips_any_tree_of_hash_widths(properties in any_properties(2)) {
        assert_round_trip(&object(properties));
    }
}
