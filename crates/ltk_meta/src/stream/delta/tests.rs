//! `bin-streaming.md` section 10.3, over synthetic bins.

use std::io;

use ltk_hash::BinHash;

use crate::{property::values, Bin, BinDelta, BinObject, Error, PropertyValueEnum};

type BinStream = crate::BinStream<io::Cursor<Vec<u8>>>;

const A: u32 = 0x1111_0001;
const B: u32 = 0x1111_0002;
const C: u32 = 0x1111_0003;
const D: u32 = 0x1111_0004;

const F_NUMBER: u32 = 0x0001;
const F_NAME: u32 = 0x0002;
const F_NODE: u32 = 0x0003;

/// Three objects with a dependency and a nested node, in file order `A`, `B`, `C`.
fn base() -> Bin {
    Bin::builder()
        .dependency("common.bin")
        .object(
            BinObject::builder(A, 0xAAAA_0001u32)
                .property(F_NUMBER, values::I32::new(42))
                .property(F_NAME, values::String::from("hello"))
                .build(),
        )
        .object(
            BinObject::builder(B, 0xAAAA_0002u32)
                .property(
                    F_NODE,
                    values::Struct {
                        class_hash: 0xAAAA_00F0u32.into(),
                        properties: [(BinHash(F_NUMBER), values::U8::new(7).into())]
                            .into_iter()
                            .collect(),
                    },
                )
                .build(),
        )
        .object(
            BinObject::builder(C, 0xAAAA_0003u32)
                .property(F_NAME, values::String::from("world"))
                .build(),
        )
        .build()
}

fn bytes_of(bin: &Bin) -> Vec<u8> {
    let mut cursor = io::Cursor::new(Vec::new());
    bin.to_writer(&mut cursor).expect("the bin writes");
    cursor.into_inner()
}

/// `bytes`, a version-3 file, rewritten as `version`. Version 1 has no dependency list, and the
/// bytes must hold none.
fn at_version(mut bytes: Vec<u8>, version: u32) -> Vec<u8> {
    bytes[4..8].copy_from_slice(&version.to_le_bytes());
    if version == 1 {
        assert_eq!(
            &bytes[8..12],
            &[0; 4],
            "a version-1 file has no dependencies"
        );
        bytes.drain(8..12);
    }
    bytes
}

fn mount(bytes: &[u8]) -> BinStream {
    BinStream::mount(io::Cursor::new(bytes.to_vec())).expect("the stream mounts")
}

fn write(bytes: &[u8], delta: &BinDelta) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    mount(bytes).write_patched(delta, &mut out)?;
    Ok(out)
}

fn read(bytes: &[u8]) -> Bin {
    Bin::from_reader(&mut io::Cursor::new(bytes)).expect("the written bin reads")
}

/// The raw bytes of every object, by path hash.
fn object_bytes(bytes: &[u8]) -> Vec<(BinHash, Vec<u8>)> {
    let mut stream = mount(bytes);
    stream
        .toc()
        .expect("the TOC builds")
        .entries()
        .iter()
        .map(|entry| {
            let range = entry.byte_range();
            (
                entry.path_hash,
                bytes[range.start as usize..range.end as usize].to_vec(),
            )
        })
        .collect()
}

#[test]
fn an_empty_delta_writes_current_format_at_every_input_version() {
    let without_dependencies = Bin::new(base().objects.into_values(), std::iter::empty::<&str>());
    for (version, bin) in [(1, &without_dependencies), (2, &base()), (3, &base())] {
        let bytes = at_version(bytes_of(bin), version);
        let delta = BinDelta::new();
        assert!(delta.is_empty());
        assert_eq!(
            write(&bytes, &delta).unwrap(),
            bytes_of(bin),
            "version {version}"
        );
    }
}

#[test]
fn a_one_property_edit_rereads_as_the_eager_edit_and_keeps_every_other_object() {
    let bytes = bytes_of(&base());

    let mut stream = mount(&bytes);
    let mut object = stream
        .object(B)
        .unwrap()
        .expect("B is in the base")
        .read()
        .unwrap();
    let PropertyValueEnum::Struct(node) = object.properties.get_mut(&BinHash(F_NODE)).unwrap()
    else {
        panic!("F_NODE is a struct");
    };
    node.properties.insert(
        BinHash(F_NAME),
        values::String::from("a longer value").into(),
    );
    let mut delta = BinDelta::new();
    assert!(delta.replace(object.clone()).is_none());
    assert_eq!(delta.replacement(B), Some(&object));
    assert!(!delta.is_empty());

    let mut out = Vec::new();
    stream.write_patched(&delta, &mut out).unwrap();

    let mut expected = base();
    expected.objects.insert(BinHash(B), object);
    assert_eq!(read(&out), expected);

    let before = object_bytes(&bytes);
    let after = object_bytes(&out);
    assert_eq!(after.len(), 3);
    for (index, (hash, body)) in after.iter().enumerate() {
        assert_eq!(*hash, before[index].0, "file order");
        if hash.0 != B {
            assert_eq!(body, &before[index].1, "{hash:08x} was rewritten");
        } else {
            assert_ne!(body, &before[index].1, "the edit is in the output");
        }
    }
}

#[test]
fn edited_output_uses_the_current_version() {
    let without_dependencies = Bin::new(base().objects.into_values(), std::iter::empty::<&str>());
    for (version, bin) in [(1, &without_dependencies), (2, &base())] {
        let bytes = at_version(bytes_of(bin), version);
        let mut stream = mount(&bytes);
        let object = stream.object(A).unwrap().unwrap().read().unwrap();
        let mut delta = BinDelta::new();
        delta.replace(object);

        let out = write(&bytes, &delta).unwrap();
        assert_eq!(out[4..8], 3u32.to_le_bytes(), "version {version}");
        assert_eq!(
            out,
            bytes_of(bin),
            "current-format output matches the eager writer"
        );
    }
}

#[test]
fn a_dependency_list_over_a_version_one_base_writes_current_format() {
    let without_dependencies = Bin::new(base().objects.into_values(), std::iter::empty::<&str>());
    let bytes = at_version(bytes_of(&without_dependencies), 1);

    let mut delta = BinDelta::new();
    delta.set_dependencies(["common.bin", "other.bin"]);
    assert_eq!(
        delta.dependencies(),
        Some(&["common.bin".to_owned(), "other.bin".to_owned()][..])
    );
    let out = write(&bytes, &delta).unwrap();
    assert_eq!(out[4..8], 3u32.to_le_bytes());
    let written = read(&out);
    assert_eq!(written.version, 3);
    assert_eq!(written.dependencies, ["common.bin", "other.bin"]);
    assert_eq!(written.objects, without_dependencies.objects);

    let mut empty = BinDelta::new();
    empty.set_dependencies(std::iter::empty::<&str>());
    assert_eq!(
        write(&bytes, &empty).unwrap(),
        bytes_of(&without_dependencies),
        "an empty list also writes current format"
    );
}

#[test]
fn removing_replacing_and_appending_update_the_class_table_and_counts() {
    let bytes = bytes_of(&base());

    let replacement = BinObject::builder(A, 0xBBBB_0001u32)
        .property(F_NUMBER, values::F32::new(1.5))
        .build();
    let appended = BinObject::builder(D, 0xBBBB_0004u32)
        .property(F_NAME, values::String::from("new"))
        .build();
    let moved = BinObject::builder(C, 0xBBBB_0003u32).build();

    let mut delta = BinDelta::new();
    delta.replace(replacement.clone());
    assert_eq!(delta.remove(B), None);
    assert!(delta.is_removed(B));
    delta.remove(C);
    assert!(delta.append(appended.clone()).is_none());
    // An appended object may take the hash of a base object the delta removes.
    delta.append(moved.clone());
    delta.set_dependencies(["patched.bin"]);

    let out = write(&bytes, &delta).unwrap();

    let mut expected = base();
    expected.objects.insert(BinHash(A), replacement);
    expected.objects.shift_remove(&BinHash(B));
    expected.objects.shift_remove(&BinHash(C));
    expected.objects.insert(BinHash(D), appended);
    expected.objects.insert(BinHash(C), moved);
    expected.dependencies = vec!["patched.bin".to_owned()];

    let written = read(&out);
    assert_eq!(written, expected);
    let stream = mount(&out);
    assert_eq!(
        stream.class_hashes(),
        [
            BinHash(0xBBBB_0001),
            BinHash(0xBBBB_0004),
            BinHash(0xBBBB_0003)
        ]
    );
    assert_eq!(
        out,
        bytes_of(&expected),
        "the same bytes the eager writer writes"
    );
}

#[test]
fn the_last_call_for_a_hash_wins() {
    let object = |value: i32| {
        BinObject::builder(A, 0xAAAA_0001u32)
            .property(F_NUMBER, values::I32::new(value))
            .build()
    };
    let mut delta = BinDelta::new();
    delta.replace(object(1));
    assert_eq!(delta.replace(object(2)), Some(object(1)));
    assert_eq!(delta.remove(A), Some(object(2)));
    assert_eq!(delta.replacement(A), None);
    delta.replace(object(3));
    assert!(!delta.is_removed(A), "a replace cancels a removal");

    let appended = |value: i32| {
        BinObject::builder(D, 0xAAAA_0004u32)
            .property(F_NUMBER, values::I32::new(value))
            .build()
    };
    delta.append(appended(1));
    assert_eq!(delta.append(appended(2)), Some(appended(1)));
    assert_eq!(delta.appended().collect::<Vec<_>>(), [&appended(2)]);
}

#[test]
fn a_delta_that_names_another_base_writes_nothing() {
    let bytes = bytes_of(&base());
    let stray = BinObject::builder(D, 0xAAAA_0004u32).build();

    let mut replaced = BinDelta::new();
    replaced.replace(stray.clone());
    let mut removed = BinDelta::new();
    removed.remove(D);
    let mut duplicate = BinDelta::new();
    duplicate.append(BinObject::builder(B, 0xAAAA_0002u32).build());
    let mut duplicate_of_replaced = BinDelta::new();
    duplicate_of_replaced.replace(BinObject::builder(B, 0xAAAA_0002u32).build());
    duplicate_of_replaced.append(BinObject::builder(B, 0xAAAA_0002u32).build());

    for (delta, expected) in [
        (&replaced, "missing"),
        (&removed, "missing"),
        (&duplicate, "duplicate"),
        (&duplicate_of_replaced, "duplicate"),
    ] {
        let mut out = Vec::new();
        let error = mount(&bytes).write_patched(delta, &mut out).unwrap_err();
        match (expected, &error) {
            ("missing", Error::DeltaMissingObject(hash)) => assert_eq!(hash.0, D),
            ("duplicate", Error::DeltaDuplicateObject(hash)) => assert_eq!(hash.0, B),
            _ => panic!("{expected}: {error:?}"),
        }
        assert!(out.is_empty(), "{expected}: bytes were written");
    }
}

#[test]
fn a_legacy_latched_base_refuses_the_delta() {
    let object = BinObject::builder(A, 0xCCCC_0001u32)
        .property(F_NODE, values::Struct::default())
        .build();
    let mut bytes = bytes_of(&Bin::new([object], std::iter::empty::<&str>()));
    // `Struct` is 19 in the legacy numbering, which decodes as nothing in the current one.
    let modern: u8 = crate::PropertyKind::Struct.into();
    let at = bytes.iter().rposition(|&byte| byte == modern).unwrap();
    bytes[at] = 19;

    let mut stream = mount(&bytes);
    stream.object(A).unwrap().unwrap().read().unwrap();
    assert!(stream.numbering().is_legacy());

    let mut out = Vec::new();
    let error = stream
        .write_patched(&BinDelta::new(), &mut out)
        .unwrap_err();
    assert!(matches!(error, Error::DeltaLegacyNumbering), "{error:?}");
    assert!(error.to_string().contains("into_bin"), "{error}");
    assert!(out.is_empty());
}

#[test]
fn a_shipped_bin_rewrites_byte_for_byte_around_one_edit() {
    const UIBASE: &[u8] = include_bytes!("../../../tests/bins/lolminimap_uibase.bin");
    let bytes = UIBASE.to_vec();
    assert_eq!(write(&bytes, &BinDelta::new()).unwrap(), bytes);

    let mut stream = mount(&bytes);
    let entries = stream.toc().unwrap().entries().to_vec();
    let target = entries[entries.len() / 2];
    let mut object = stream
        .object(target.path_hash)
        .unwrap()
        .unwrap()
        .read()
        .unwrap();
    object
        .properties
        .insert(BinHash(0x0BAD_F00D), values::Bool::new(true).into());
    let mut delta = BinDelta::new();
    delta.replace(object.clone());
    let mut out = Vec::new();
    stream.write_patched(&delta, &mut out).unwrap();

    let mut expected = read(&bytes);
    expected.objects.insert(target.path_hash, object);
    assert_eq!(read(&out), expected);
    let before = object_bytes(&bytes);
    let after = object_bytes(&out);
    for ((hash, body), (_, original)) in after.iter().zip(&before) {
        if *hash != target.path_hash {
            assert_eq!(body, original, "{hash:08x}");
        }
    }
}

#[test]
fn unread_legacy_objects_refuse_output_before_any_bytes_are_written() {
    let legacy = BinObject::builder(B, 0xCCCC_0001u32)
        .property(F_NODE, values::Struct::default())
        .build();
    let prefix = BinObject::builder(A, 0xAAAA_0001u32)
        .property(F_NUMBER, values::I32::new(42))
        .build();
    let mut bytes = bytes_of(&Bin::new([prefix, legacy], std::iter::empty::<&str>()));
    let tag = mount(&bytes)
        .toc()
        .unwrap()
        .entry(BinHash(B))
        .unwrap()
        .offset as usize
        + 14;
    assert_eq!(bytes[tag], u8::from(crate::PropertyKind::Struct));
    bytes[tag] = 19;

    let mut appended = BinDelta::new();
    appended.append(
        BinObject::builder(D, 0xDDDD_0001u32)
            .property(F_NODE, values::Struct::default())
            .build(),
    );
    let mut replaced = appended.clone();
    replaced.replace(BinObject::builder(B, 0xCCCC_0001u32).build());
    let mut removed = appended.clone();
    removed.remove(B);
    for delta in [BinDelta::new(), appended, replaced, removed] {
        let mut stream = mount(&bytes);
        stream.object(A).unwrap().unwrap().read().unwrap();
        assert!(!stream.numbering().is_legacy());
        let mut out = Vec::new();
        assert!(matches!(
            stream.write_patched(&delta, &mut out),
            Err(Error::DeltaLegacyNumbering)
        ));
        assert!(out.is_empty());
    }
}

#[test]
fn malformed_untouched_objects_refuse_output_before_any_bytes_are_written() {
    let mut bytes = bytes_of(&base());
    let entry = mount(&bytes).toc().unwrap().entries()[0];
    let count = entry.offset as usize + 8;
    bytes[count..count + 2].copy_from_slice(&0u16.to_le_bytes());
    let mut out = Vec::new();
    assert!(matches!(
        mount(&bytes).write_patched(&BinDelta::new(), &mut out),
        Err(Error::InvalidSize(..))
    ));
    assert!(out.is_empty());
}
