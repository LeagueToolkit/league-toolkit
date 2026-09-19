//! `PTCH` text: parsing to a [`BinOverride`], printing one back, and the diagnostics in between.

use std::io::Cursor;

use glam::{Mat4, Vec2, Vec3, Vec4};
use ltk_hash::{BinHash, Hash as _};
use ltk_meta::{
    path::{PropertyPath, PropertyPathErrorKind},
    property::{values, NoMeta},
    Bin, BinFile, BinObject, BinOverride, PropertyKind, PropertyPatch,
};
use ltk_primitives::Color;
use ltk_ritobin::{
    ast::{
        diagnostics::{Diagnostic, DiagnosticWithSpan, PatchField, RitoTypeOrVirtual},
        node::root::{FileKind, RootKind},
    },
    print::Print as _,
    Cst,
};

/// Builds `text` as a whole file and asserts it produced no diagnostics.
fn build_clean(text: &str) -> BinFile {
    let cst = Cst::parse(text);
    assert!(cst.errors.is_empty(), "parse errors: {:#?}", cst.errors);
    let (file, diagnostics) = cst.build(text);
    assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:#?}");
    file
}

/// Builds `text` as a whole file, returning the file and its diagnostics.
fn build(text: &str) -> (BinFile, Vec<DiagnosticWithSpan>) {
    let cst = Cst::parse(text);
    assert!(cst.errors.is_empty(), "parse errors: {:#?}", cst.errors);
    cst.build(text)
}

/// The source text each diagnostic points at, next to the diagnostic.
fn located<'a>(text: &'a str, diagnostics: &[DiagnosticWithSpan]) -> Vec<(&'a str, Diagnostic)> {
    diagnostics
        .iter()
        .map(|d| (&text[d.span], d.diagnostic))
        .collect()
}

/// A `PTCH` file with `records` as the body of its `patches` root.
fn ptch(records: &str) -> String {
    format!(
        r#"#PROP_text
type: string = "PTCH"
version: u32 = 3
linked: list[string] = {{}}
entries: map[hash,embed] = {{}}
patches: map[hash,embed] = {{
{records}
}}
"#
    )
}

/// Builds a `PTCH` file of `records`, asserts it produced exactly one diagnostic pointing at
/// `at`, and returns that diagnostic and the number of records that reached the patch.
fn one_diagnostic(records: &str, at: &str) -> (Diagnostic, usize) {
    let text = ptch(records);
    let (file, diagnostics) = build(&text);
    let diagnostics = located(&text, &diagnostics);
    let [(span, diagnostic)] = diagnostics[..] else {
        panic!("expected one diagnostic, got {diagnostics:#?}");
    };
    assert_eq!(span, at, "{diagnostic:#?}");
    let patch = file
        .into_override()
        .expect("a PTCH file builds a BinOverride");
    (diagnostic, patch.patches.len())
}

fn path(text: &str) -> PropertyPath {
    PropertyPath::new(text).unwrap()
}

#[test]
fn the_ticket_example_parses_into_two_records_in_authored_order() {
    let text = r#"#PROP_text
type: string = "PTCH"
version: u32 = 3
linked: list[string] = {}
entries: map[hash,embed] = {}
patches: map[hash,embed] = {
    0x4a47c414 = patch {
        path: string = "Position.Anchors.Anchor"
        value: vec2 = { 0, 1 }
    }
    0x4a47c414 = patch {
        path: string = "Position.Anchors.Anchor"
        value: vec2 = { 1, 0 }
    }
}
"#;

    let file = build_clean(text);

    let expected = BinOverride::builder()
        .patch(PropertyPatch::new(
            0x4a47c414,
            path("Position.Anchors.Anchor"),
            values::Vector2::new(Vec2::new(0.0, 1.0)),
        ))
        .patch(PropertyPatch::new(
            0x4a47c414,
            path("Position.Anchors.Anchor"),
            values::Vector2::new(Vec2::new(1.0, 0.0)),
        ))
        .build();
    pretty_assertions::assert_eq!(file, BinFile::Override(expected));
}

#[test]
fn build_bin_diagnoses_a_ptch_file() {
    let text = ptch(
        r#"    0x4a47c414 = patch {
        path: string = "FlipX"
        value: bool = true
    }"#,
    );
    let cst = Cst::parse(&text);

    let partial = cst.build_bin(&text);

    let diagnostics = located(&text, &partial.diagnostics);
    assert!(
        matches!(
            diagnostics[..],
            [(
                "\"PTCH\"",
                Diagnostic::UnexpectedFileKind {
                    expected: FileKind::Prop,
                    found: FileKind::Patch,
                    ..
                }
            )]
        ),
        "{diagnostics:#?}"
    );
    assert!(partial.into_result().is_err());
}

#[test]
fn ptch_roots_on_a_prop_file_are_diagnosed() {
    let text = r#"#PROP_text
type: string = "PROP"
version: u32 = 3
linked: list[string] = {}
entries: map[hash,embed] = {}
patches: map[hash,embed] = {
    0x1 = patch {
        path: string = "FlipX"
    }
}
deleted: list[hash] = { 0xdeadbeef }
"#;

    let (file, diagnostics) = build(text);

    let diagnostics = located(text, &diagnostics);
    assert!(
        matches!(
            diagnostics[..],
            [
                (
                    "patches",
                    Diagnostic::PatchOnlyRoot {
                        root_kind: RootKind::Patches,
                        ..
                    }
                ),
                (
                    "deleted",
                    Diagnostic::PatchOnlyRoot {
                        root_kind: RootKind::Deleted,
                        ..
                    }
                ),
            ]
        ),
        "{diagnostics:#?}"
    );
    assert!(file.is_prop());
}

#[test]
fn a_ptch_file_that_links_other_bins_is_diagnosed() {
    let text = ptch("").replace(
        "linked: list[string] = {}",
        r#"linked: list[string] = { "common.bin" }"#,
    );

    let (_, diagnostics) = build(&text);

    let diagnostics = located(&text, &diagnostics);
    assert!(
        matches!(
            diagnostics[..],
            [(r#"{ "common.bin" }"#, Diagnostic::PatchLinked { .. })]
        ),
        "{diagnostics:#?}"
    );
}

#[test]
fn a_ptch_file_of_another_text_version_is_diagnosed() {
    let text = ptch("").replace("version: u32 = 3", "version: u32 = 2");

    let (_, diagnostics) = build(&text);

    let diagnostics = located(&text, &diagnostics);
    assert!(
        matches!(
            diagnostics[..],
            [("2", Diagnostic::UnsupportedPatchVersion { version: 2, .. })]
        ),
        "{diagnostics:#?}"
    );
}

#[test]
fn a_record_that_is_not_a_patch_embed_is_diagnosed() {
    let (diagnostic, records) = one_diagnostic(
        r#"    0x1 = Patch2 {
        path: string = "FlipX"
        value: bool = true
    }"#,
        "Patch2",
    );
    assert!(
        matches!(diagnostic, Diagnostic::UnexpectedPatchClass { .. }),
        "{diagnostic:#?}"
    );
    assert_eq!(
        records, 1,
        "a record of another class keeps its path and value"
    );
}

#[test]
fn a_record_without_a_path_is_diagnosed_and_left_out() {
    let (diagnostic, records) = one_diagnostic(
        r#"    0x1 = patch {
        value: bool = true
    }"#,
        "patch",
    );
    assert!(
        matches!(
            diagnostic,
            Diagnostic::MissingPatchField {
                field: PatchField::Path,
                ..
            }
        ),
        "{diagnostic:#?}"
    );
    assert_eq!(records, 0);
}

#[test]
fn a_record_without_a_value_is_diagnosed_and_left_out() {
    let (diagnostic, records) = one_diagnostic(
        r#"    0x1 = patch {
        path: string = "FlipX"
    }"#,
        "patch",
    );
    assert!(
        matches!(
            diagnostic,
            Diagnostic::MissingPatchField {
                field: PatchField::Value,
                ..
            }
        ),
        "{diagnostic:#?}"
    );
    assert_eq!(records, 0);
}

#[test]
fn a_record_with_two_paths_is_diagnosed_at_the_second() {
    let (diagnostic, _) = one_diagnostic(
        r#"    0x1 = patch {
        path: string = "FlipX"
        value: bool = true
        Path: string = "FlipY"
    }"#,
        "Path",
    );
    assert!(
        matches!(
            diagnostic,
            Diagnostic::DuplicatePatchField {
                field: PatchField::Path,
                ..
            }
        ),
        "{diagnostic:#?}"
    );
}

#[test]
fn a_record_with_two_values_is_diagnosed_at_the_second() {
    let (diagnostic, _) = one_diagnostic(
        r#"    0x1 = patch {
        path: string = "FlipX"
        value: bool = true
        value: bool = false
    }"#,
        "value",
    );
    assert!(
        matches!(
            diagnostic,
            Diagnostic::DuplicatePatchField {
                field: PatchField::Value,
                ..
            }
        ),
        "{diagnostic:#?}"
    );
}

#[test]
fn a_record_with_another_field_is_diagnosed() {
    let (diagnostic, _) = one_diagnostic(
        r#"    0x1 = patch {
        path: string = "FlipX"
        value: bool = true
        kind: u8 = 1
    }"#,
        "kind: u8 = 1",
    );
    assert!(
        matches!(diagnostic, Diagnostic::UnexpectedPatchField { .. }),
        "{diagnostic:#?}"
    );
}

#[test]
fn a_path_that_is_not_a_string_is_diagnosed_and_left_out() {
    let (diagnostic, records) = one_diagnostic(
        r#"    0x1 = patch {
        path: hash = "FlipX"
        value: bool = true
    }"#,
        r#""FlipX""#,
    );
    assert!(
        matches!(
            diagnostic,
            Diagnostic::TypeMismatch {
                got: RitoTypeOrVirtual::RitoType(got),
                ..
            } if got.base == PropertyKind::Hash
        ),
        "{diagnostic:#?}"
    );
    assert_eq!(records, 0);
}

#[test]
fn an_invalid_property_path_is_diagnosed_and_left_out() {
    let (diagnostic, records) = one_diagnostic(
        r#"    0x1 = patch {
        path: string = "Elements[3]x"
        value: bool = true
    }"#,
        r#""Elements[3]x""#,
    );
    let Diagnostic::InvalidPropertyPath { error, .. } = diagnostic else {
        panic!("{diagnostic:#?}");
    };
    assert_eq!(
        error.kind(),
        PropertyPathErrorKind::UnexpectedCharacter('x')
    );
    assert_eq!(records, 0);
}

#[test]
fn an_invalid_typed_value_is_diagnosed() {
    let (diagnostic, _) = one_diagnostic(
        r#"    0x1 = patch {
        path: string = "Position"
        value: vec2 = { 0 }
    }"#,
        " 0",
    );
    assert!(
        matches!(diagnostic, Diagnostic::NotEnoughItems { got: 1, .. }),
        "{diagnostic:#?}"
    );
}

#[test]
fn a_record_key_that_is_not_a_hash_is_diagnosed_and_left_out() {
    let (diagnostic, records) = one_diagnostic(
        r#"    true = patch {
        path: string = "FlipX"
        value: bool = true
    }"#,
        "true",
    );
    assert!(
        matches!(diagnostic, Diagnostic::TypeMismatch { .. }),
        "{diagnostic:#?}"
    );
    assert_eq!(records, 0);
}

#[test]
fn an_absent_patches_root_is_zero_records() {
    let text = r#"#PROP_text
type: string = "PTCH"
version: u32 = 3
linked: list[string] = {}
entries: map[hash,embed] = {}
"#;

    let file = build_clean(text);

    assert_eq!(file, BinFile::Override(BinOverride::default()));
}

#[test]
fn an_empty_patches_root_is_zero_records() {
    let file = build_clean(&ptch(""));

    assert_eq!(file, BinFile::Override(BinOverride::default()));
}

#[test]
fn the_deleted_root_lists_the_objects_a_patch_deletes() {
    let text = ptch("").replace(
        "entries: map[hash,embed] = {}",
        "entries: map[hash,embed] = {}\ndeleted: list[hash] = { 0xdeadbeef, \"Some/Object\" }",
    );

    let file = build_clean(&text);

    let expected = BinOverride::builder()
        .delete(0xdeadbeef)
        .delete(0xee5409d1_u32) // FNV-1a of "some/object"
        .build();
    assert_eq!(file, BinFile::Override(expected));
}

#[test]
fn a_patch_prints_its_records_under_patches_and_its_deletions_last() {
    let patch = BinOverride::builder()
        .delete(0xdeadbeef)
        .set(
            0x4a47c414,
            path("Position.Anchors.Anchor"),
            values::Vector2::new(Vec2::new(0.0, 1.0)),
        )
        .build();

    let text = patch.print().unwrap();

    pretty_assertions::assert_eq!(
        text,
        r#"#PROP_text
type: string = "PTCH"
version: u32 = 3
linked: list[string] = { }
entries: map[hash, embed] = { }
patches: map[hash, embed] = {
    0x4a47c414 = patch {
        path: string = "Position.Anchors.Anchor"
        value: vec2 = { 0, 1 }
    }
}
deleted: list[hash] = { 0xdeadbeef }"#
    );
}

#[test]
fn a_patch_without_deletions_prints_no_deleted_root() {
    let patch = BinOverride::builder()
        .set(0x1, path("FlipX"), values::Bool::new(true))
        .build();

    let text = patch.print().unwrap();

    assert!(!text.contains("deleted"), "{text}");
}

#[test]
fn a_bin_file_prints_as_the_bin_it_holds() {
    let patch = BinOverride::builder()
        .set(0x1, path("FlipX"), values::Bool::new(true))
        .build();

    let text = BinFile::Override(patch.clone()).print().unwrap();

    assert_eq!(text, patch.print().unwrap());
}

/// The flipped minimap from `UI.wad.client` of client 16.16.804.9184: 0 objects, 109 records.
const UIFLIPPED: &[u8] = include_bytes!("../../ltk_meta/tests/bins/lolminimap_uiflipped.ptch.bin");

#[test]
fn a_patch_with_added_objects_and_mixed_records_survives_print_and_parse() {
    let embed = values::Embedded(values::Struct {
        class_hash: 0x4eb9ba4f.into(),
        meta: NoMeta,
        properties: [
            (
                0x934f4e0a.into(),
                values::Vector2::new(Vec2::new(368.0, 1168.0)).into(),
            ),
            (0xf6647cd6.into(), values::U16::new(1600).into()),
        ]
        .into_iter()
        .collect(),
    });
    let pointer = values::Struct {
        class_hash: 0x0a5d0595.into(),
        meta: NoMeta,
        properties: [(0x02f3b39e.into(), values::Bool::new(true).into())]
            .into_iter()
            .collect(),
    };
    let patch = BinOverride::builder()
        .delete(0xdeadbeef)
        .delete(0x0000c651)
        .object(
            BinObject::<NoMeta>::builder(0x472d1ae4, 0x0202c6c9)
                .property(0x07a640f6, values::U32::new(39))
                .build(),
        )
        .set(0x1, path("Enabled"), values::Bool::new(false))
        .set(0x1, path("Visible"), values::BitBool::new(true))
        .set(0x1, path("Alpha"), values::U8::new(200))
        .set(0x1, path("Offset8"), values::I8::new(-8))
        .set(0x1, path("Width"), values::U16::new(1600))
        .set(0x1, path("Offset16"), values::I16::new(-16))
        .set(0x1, path("Flags"), values::U32::new(0x8000_0001))
        .set(0x1, path("Order"), values::I32::new(-4))
        .set(0x1, path("Seed"), values::U64::new(u64::MAX))
        .set(0x1, path("Delta"), values::I64::new(i64::MIN))
        .set(
            0x1,
            path("Anchor"),
            values::Vector2::new(Vec2::new(0.0, 1.0)),
        )
        .set(0x1, path("Nothing"), values::None::default())
        .set(0x1, path("Scale"), values::F32::new(0.25))
        .set(0x1, path("Name"), values::String::from("minimap"))
        .set(0x1, path("Id"), values::Hash::new(0x3b9c7079))
        .set(0x1, path("Target"), values::ObjectLink::new(0x17566805))
        .set(
            0x1,
            path("Texture"),
            values::WadChunkLink::new(0x0123456789abcdef),
        )
        .set(0x2, path("Position.UIRect"), embed)
        .set(0x2, path("Position.Anchors"), pointer)
        .set(
            0x2,
            path("Offset"),
            values::Vector3::new(Vec3::new(1.0, -2.5, 3.0)),
        )
        .set(
            0x2,
            path("Bounds"),
            values::Vector4::new(Vec4::new(0.0, 0.5, 1.0, 1.5)),
        )
        .set(
            0x2,
            path("Tint"),
            values::Color::new(Color {
                r: 255,
                g: 128,
                b: 0,
                a: 64,
            }),
        )
        .set(
            0x2,
            path("Transform"),
            values::Matrix44::new(Mat4::IDENTITY),
        )
        .set(
            0x3,
            path("Elements[3]"),
            values::Container::from(vec![values::ObjectLink::new(0x1ed62b1)]),
        )
        .set(
            0x3,
            path("Lookup{\"weapon\"}"),
            values::Map::new(
                PropertyKind::Hash,
                PropertyKind::String,
                vec![(
                    values::Hash::new(0x5ee5ea9a).into(),
                    values::String::from("sword").into(),
                )],
            )
            .unwrap(),
        )
        .set(
            0x3,
            path("Width"),
            values::Optional::from(values::F32::new(310.0)),
        )
        .set(
            0x3,
            path("Tags"),
            values::UnorderedContainer(values::Container::from(vec![
                values::Hash::new(0x1),
                values::Hash::new(0x2),
            ])),
        )
        .build();

    let text = patch.print().unwrap();
    let file = build_clean(&text);

    pretty_assertions::assert_eq!(file, BinFile::Override(patch), "{text}");
}

#[test]
fn text_compiles_to_bytes_that_ltk_meta_reads_back() {
    let text = ptch(
        r#"    0x4a47c414 = patch {
        path: string = "Position.Anchors.Anchor"
        value: vec2 = { 0, 1 }
    }
    0xa4edcb0d = patch {
        path: string = "FlipX"
        value: bool = true
    }"#,
    );
    let patch = build_clean(&text).into_override().unwrap();

    let mut bytes = Cursor::new(Vec::new());
    patch.to_writer(&mut bytes).unwrap();
    bytes.set_position(0);
    let read = BinOverride::from_reader(&mut bytes).unwrap();

    assert_eq!(read, patch);
}

#[test]
fn the_shipped_patch_survives_text_and_binary_round_trips() {
    let shipped = BinOverride::from_reader(&mut Cursor::new(UIFLIPPED)).unwrap();
    assert_eq!(shipped.patches.len(), 109);

    let text = shipped.print().unwrap();
    assert!(!text.contains("deleted"), "{text}");
    let reparsed = build_clean(&text).into_override().unwrap();
    pretty_assertions::assert_eq!(reparsed, shipped);

    let mut bytes = Cursor::new(Vec::new());
    reparsed.to_writer(&mut bytes).unwrap();
    assert!(
        bytes.into_inner() == UIFLIPPED,
        "text round trip changed the binary"
    );
}

#[test]
fn repeated_records_apply_in_the_order_they_are_written() {
    let anchor = BinHash::hash_str("Anchor");
    let mut base = Bin::builder()
        .object(
            BinObject::<NoMeta>::builder(0x4a47c414, 0x1234)
                .property(anchor, values::Vector2::new(Vec2::new(1.0, 1.0)))
                .build(),
        )
        .build();
    let text = ptch(
        r#"    0x4a47c414 = patch {
        path: string = "Anchor"
        value: vec2 = { 0, 1 }
    }
    0x4a47c414 = patch {
        path: string = "Anchor"
        value: vec2 = { 1, 0 }
    }"#,
    );
    let patch = build_clean(&text).into_override().unwrap();

    patch.apply(&mut base);

    let anchor = base.objects[&BinHash::from(0x4a47c414)]
        .resolve(&path("Anchor"))
        .unwrap();
    assert_eq!(anchor, &values::Vector2::new(Vec2::new(1.0, 0.0)).into());
}

#[test]
fn a_parse_error_displays_what_the_parser_expected() {
    let text = ptch("").replace("version: u32 = 3", "version: u32 = \"3");

    let cst = Cst::parse(&text);

    let error = cst.errors.first().expect("an unterminated string");
    assert_eq!(error.to_string(), "Unterminated string");
}

#[test]
fn a_record_with_a_parse_error_is_a_parse_error_not_a_diagnostic() {
    let text = ptch(
        r#"    0x1 = patch {
        path: string = "FlipX"
        value: bool = true
    }
    0x2 = patch {
        path: string = "FlipY"
        value: bool = @@@
    }"#,
    );

    let cst = Cst::parse(&text);

    assert!(!cst.errors.is_empty(), "the second record does not parse");
}
