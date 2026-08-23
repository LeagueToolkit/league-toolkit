//! Tests for `PTCH` bins.

use std::io::{Cursor, Seek};

use indexmap::IndexMap;
use insta::assert_ron_snapshot;
use ltk_meta::{
    path::PropertyPath,
    property::{values, Kind, NoMeta},
    Bin, BinFile, BinKind, BinObject, BinOverride, Error, PropertyPatch, PropertyValueEnum,
};

/// A flipped minimap patch from `UI.wad.client` of client 16.16.804.9184: no deletions, no
/// objects, 109 patch records.
const UIFLIPPED: &[u8] = include_bytes!("bins/lolminimap_uiflipped.ptch.bin");

fn path(text: &str) -> PropertyPath {
    PropertyPath::new(text).unwrap()
}

fn write(patch_bin: &BinOverride) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    patch_bin.to_writer(&mut out).expect("write failed");
    out.into_inner()
}

fn read(bytes: &[u8]) -> Result<BinOverride, Error> {
    BinOverride::from_reader(&mut Cursor::new(bytes))
}

/// A patch covering every shape a record can take, including one of each container kind.
fn sample_patch_bin() -> BinOverride {
    let mut properties = IndexMap::new();
    properties.insert(0x1111_u32.into(), values::I32::new(42).into());

    BinOverride::builder()
        .delete(0xdead_beef_u32)
        .deletions([0x1234_u32, 0x5678])
        .object(
            BinObject::<NoMeta>::builder(0x1111_2222_u32, 0x3333_4444)
                .property(0xaaaa_u32, values::String::from("hello"))
                .build(),
        )
        .set(0x4a47_c414, path("Position.Anchors.Anchor"), {
            values::Vector2::new(glam::Vec2::new(0.0, 1.0))
        })
        .set(0xa4ed_cb0d, path("FlipX"), values::Bool::new(true))
        .set(0x0001_u32, path("Name"), values::String::from("a string"))
        .set(0x0002_u32, path("Elements[3]"), values::U16::new(7))
        .set(
            0x0003_u32,
            path(r#"PerAttachmentMaterial{"weapon"}"#),
            values::Hash::new(0xcafe_babe_u32),
        )
        .set(
            0x0004_u32,
            path("Items"),
            values::Container::from(vec![values::F32::new(1.5), values::F32::new(2.5)]),
        )
        .set(
            0x0005_u32,
            path("Lookup"),
            values::Map::new(
                Kind::U32,
                Kind::String,
                vec![(
                    values::U32::new(1).into(),
                    values::String::from("one").into(),
                )],
            )
            .unwrap(),
        )
        .set(
            0x0006_u32,
            path("Maybe"),
            values::Optional::from(values::I32::new(-1)),
        )
        .set(
            0x0007_u32,
            path("Position.UIRect"),
            values::Embedded(values::Struct {
                class_hash: 0x4eb9_ba4f.into(),
                properties: properties.clone(),
                meta: Default::default(),
            }),
        )
        .patch(PropertyPatch::new(
            0x0008_u32,
            path("Pointer"),
            values::Struct {
                class_hash: 0x1234_5678.into(),
                properties,
                meta: Default::default(),
            },
        ))
        .build()
}

#[test]
fn reads_a_shipped_patch_bin() {
    let patch_bin = read(UIFLIPPED).unwrap();

    assert!(patch_bin.deleted.is_empty());
    assert!(patch_bin.objects.is_empty());
    assert_eq!(patch_bin.patches.len(), 109);
    assert!(!patch_bin.is_empty());

    let first = &patch_bin.patches[0];
    assert_eq!(first.object_hash, 0x4a47_c414.into());
    assert_eq!(first.path.as_str(), "Position.Anchors.Anchor");
    assert_eq!(first.kind(), Kind::Vector2);
    assert_eq!(
        first.value,
        PropertyValueEnum::Vector2(values::Vector2::new(glam::Vec2::new(0.0, 1.0)))
    );

    // The byte-identical rewrite below is the real guard on the parse; this is here to keep the
    // shape of a record readable.
    insta::with_settings!({sort_maps => true}, {
        assert_ron_snapshot!(&patch_bin.patches[..3]);
    });
}

#[test]
fn rewrites_a_shipped_patch_bin_byte_for_byte() {
    let patch_bin = read(UIFLIPPED).unwrap();
    assert_eq!(write(&patch_bin), UIFLIPPED);
}

#[test]
fn round_trips_a_built_patch_bin() {
    let patch_bin = sample_patch_bin();
    let bytes = write(&patch_bin);

    assert_eq!(read(&bytes).unwrap(), patch_bin);
}

#[test]
fn reads_either_kind_of_file() {
    let prop = include_bytes!("bins/leona_small.bin");

    let file = BinFile::from_reader(&mut Cursor::new(UIFLIPPED)).unwrap();
    assert_eq!(file.kind(), BinKind::Override);
    assert!(file.is_override());
    assert_eq!(file.as_override().unwrap().patches.len(), 109);
    assert!(file.as_prop().is_none());
    assert!(file.objects().is_empty());
    assert!(file.into_override().is_some());

    let mut file = BinFile::from_reader(&mut Cursor::new(prop)).unwrap();
    assert_eq!(file.kind(), BinKind::Prop);
    assert!(file.is_prop());
    assert!(file.as_override().is_none());
    assert_eq!(file.objects().len(), file.as_prop().unwrap().objects.len());
    assert!(!file.objects().is_empty());
    file.objects_mut().clear();
    assert!(file.as_prop_mut().unwrap().objects.is_empty());

    // Whichever kind, it writes back out in its own format.
    let file = BinFile::from(read(UIFLIPPED).unwrap());
    let mut out = Cursor::new(Vec::new());
    file.to_writer(&mut out).unwrap();
    assert_eq!(out.get_ref().as_slice(), UIFLIPPED);
    out.rewind().unwrap();
    assert_eq!(BinFile::from_reader(&mut out).unwrap(), file);
}

#[test]
fn tells_the_caller_which_reader_to_use() {
    let prop = include_bytes!("bins/leona_small.bin");

    assert_eq!(BinKind::identify_from_bytes(prop), Some(BinKind::Prop));
    assert_eq!(
        BinKind::identify_from_bytes(UIFLIPPED),
        Some(BinKind::Override)
    );
    assert_eq!(BinKind::identify_from_bytes(b"OEGM"), None);
    assert_eq!(BinKind::identify_from_bytes(b"PTC"), None);

    // The magic is left in place, so the reader it names can be handed the same reader.
    let mut reader = Cursor::new(UIFLIPPED);
    match BinKind::identify_from_reader(&mut reader).unwrap() {
        BinKind::Prop => panic!("that is a PTCH"),
        BinKind::Override => {
            assert_eq!(reader.position(), 0);
            assert_eq!(
                BinOverride::from_reader(&mut reader).unwrap().patches.len(),
                109
            );
        }
    }

    let mut reader = Cursor::new(prop);
    assert_eq!(
        BinKind::identify_from_reader(&mut reader).unwrap(),
        BinKind::Prop
    );
    assert!(Bin::from_reader(&mut reader).is_ok());

    assert!(matches!(
        BinKind::identify_from_reader(&mut Cursor::new(b"NOPE\0\0\0\0")),
        Err(Error::InvalidFileSignature)
    ));
}

#[test]
fn rejects_the_other_kind_of_file() {
    let prop = include_bytes!("bins/leona_small.bin");

    assert!(matches!(
        read(prop),
        Err(Error::UnexpectedBinKind {
            expected: BinKind::Override,
            found: BinKind::Prop
        })
    ));
    assert!(matches!(
        Bin::from_reader(&mut Cursor::new(UIFLIPPED)),
        Err(Error::UnexpectedBinKind {
            expected: BinKind::Prop,
            found: BinKind::Override
        })
    ));
    assert!(matches!(
        read(b"NOPE\0\0\0\0"),
        Err(Error::InvalidFileSignature)
    ));
}

// The header is PTCH, version, delete count, the deleted hashes, PROP, version, dependency
// count, object count. The patch count follows the objects.
const OVERRIDE_VERSION: usize = 4;

/// Where the inner `PROP` section starts.
fn prop_at(patch_bin: &BinOverride) -> usize {
    12 + 4 * patch_bin.deleted.len()
}

fn corrupt(bytes: &[u8], at: usize, value: u32) -> Vec<u8> {
    let mut bytes = bytes.to_vec();
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    bytes
}

#[test]
fn rejects_a_patch_bin_the_client_could_not_load() {
    let patch_bin = sample_patch_bin();
    let bytes = write(&patch_bin);
    let prop = prop_at(&patch_bin);

    assert!(matches!(
        read(&corrupt(&bytes, OVERRIDE_VERSION, 2)),
        Err(Error::InvalidOverrideVersion(2))
    ));
    assert!(matches!(
        read(&corrupt(&bytes, prop + 4, 4)),
        Err(Error::InvalidFileVersion(4))
    ));
    // The client reads the dependency count and never skips the strings behind it.
    assert!(matches!(
        read(&corrupt(&bytes, prop + 8, 1)),
        Err(Error::OverrideDependencies(1))
    ));
}

#[test]
fn rejects_a_record_that_does_not_add_up() {
    let patch_bin = BinOverride::builder()
        .set(0x1234_u32, path("ABCDE"), values::Bool::new(true))
        .build();
    let bytes = write(&patch_bin);

    // With no deletions and no objects the patch count sits at byte 28, and the one record
    // follows it: object hash, payload size, kind, path length, path.
    let size = 28 + 8;
    let payload_size = u32::from_le_bytes(bytes[size..size + 4].try_into().unwrap());
    assert!(matches!(
        read(&corrupt(&bytes, size, payload_size + 1)),
        Err(Error::InvalidSize(..))
    ));

    // Same length, so only the path itself becomes invalid.
    let mut bad_path = bytes.clone();
    let at = bad_path.len() - 6;
    bad_path[at..at + 5].copy_from_slice(b"ABC..");
    assert!(matches!(
        read(&bad_path),
        Err(Error::InvalidPropertyPath { index: 0, object_hash, .. })
            if object_hash == 0x1234_u32.into()
    ));
}

#[test]
fn writes_the_shape_the_client_expects() {
    let bytes = write(&BinOverride::default());

    assert_eq!(&bytes[0..4], b"PTCH");
    assert_eq!(&bytes[4..8], 1_u32.to_le_bytes());
    assert_eq!(&bytes[8..12], 0_u32.to_le_bytes());
    assert_eq!(&bytes[12..16], b"PROP");
    assert_eq!(&bytes[16..20], 3_u32.to_le_bytes());
    assert_eq!(&bytes[20..24], 0_u32.to_le_bytes());
    assert_eq!(&bytes[24..28], 0_u32.to_le_bytes());
    assert_eq!(&bytes[28..32], 0_u32.to_le_bytes());
    assert_eq!(bytes.len(), 32);

    assert!(BinOverride::<NoMeta>::default().is_empty());
}

#[test]
fn serializes_paths_as_strings() {
    let patch = PropertyPatch::<NoMeta>::new(0x1234_u32, path("A.B[1]"), values::Bool::new(true));
    let text = serde_json::to_string(&patch.path).unwrap();

    assert_eq!(text, r#""A.B[1]""#);
    assert_eq!(
        serde_json::from_str::<PropertyPath>(&text).unwrap(),
        patch.path
    );
    assert!(serde_json::from_str::<PropertyPath>(r#""A..B""#).is_err());
}
