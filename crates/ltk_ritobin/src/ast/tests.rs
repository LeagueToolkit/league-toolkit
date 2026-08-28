use glam::{Vec3, Vec4};
use ltk_hash::BinHash;
use ltk_meta::{
    property::{values, NoMeta},
    Bin, BinObject, ObjectBuilder, PropertyKind, PropertyValueEnum,
};

use crate::{
    ast::{
        builder::RootKind,
        diagnostics::{Diagnostic, DiagnosticWithSpan},
    },
    Cst, ItemShape, RitoType,
};

fn wrap(input: &str) -> String {
    format!(
        r#"
#PROP_text
type: string = "PROP"
version: u32 = 3
linked: list[string] = {{}}
entries: map[hash,embed] = {{
    0xDEADBEEF = 0x1234123 {{
        {input}
    }}
}}"#
    )
}

fn assert<F: Fn(ObjectBuilder) -> ObjectBuilder>(input: &str, is: F) {
    let input = wrap(input);

    let cst = Cst::parse(&input);
    let mut str = String::new();

    cst.print(&mut str, &input);
    eprintln!("#### CST:\n{str}");

    let ast = cst.build_ast(&input);

    assert!(
        ast.diagnostics.is_empty(),
        "Typecheck errors: {:#?}",
        ast.diagnostics
    );
    let bin = ast.to_bin(&input);

    let obj = (is)(BinObject::<NoMeta>::builder(0xDEADBEEF, 0x1234123)).build();
    pretty_assertions::assert_eq!(bin, Bin::builder().object(obj).build());
}

/// Builds a full object body (see [`wrap`]) from `input` and returns the
/// typecheck diagnostics without asserting they're empty - for exercising
/// error paths.
fn build_errs(input: &str) -> Vec<DiagnosticWithSpan> {
    let input = wrap(input);
    let cst = Cst::parse(&input);

    if option_env!("PRINT_CST").is_some() {
        let mut str = String::new();
        cst.print(&mut str, &input);
        eprintln!("####\n{str}\n#####");
    }

    cst.build_ast(&input).diagnostics
}

#[test]
fn option() {
    assert(r#"0x1: option[vec3] = { { 0.5, 5.3, -0.20 } }"#, |obj| {
        obj.property(
            0x1,
            values::Optional::from(values::Vector3::from(Vec3::new(0.5, 5.3, -0.2))),
        )
    });
}
#[test]
fn option_coerce() {
    assert(r#"0x1: option[vec3] = { 0.5, 5.3, -0.20 }"#, |obj| {
        obj.property(
            0x1,
            values::Optional::from(values::Vector3::from(Vec3::new(0.5, 5.3, -0.2))),
        )
    });
}

#[test]
fn list() {
    assert(
        r#"
    values: list[vec4] = {
        { 1, 1, 1, 1 }
        { 1, 1, 1, 1 }
        { 1, 1, 1, 0 }
    }
    "#,
        |obj| {
            obj.property(
                0x34474c3b,
                values::Container::from_iter([
                    values::Vector4::from(Vec4::new(1., 1., 1., 1.)),
                    values::Vector4::from(Vec4::new(1., 1., 1., 1.)),
                    values::Vector4::from(Vec4::new(1., 1., 1., 0.)),
                ]),
            )
        },
    );
}

#[test]
fn u8_map() {
    assert(
        r#"
    0xe6d60f41: map[u8,string] = {
        1 = "hello"
    }
    "#,
        |obj| {
            obj.property(
                0xe6d60f41,
                values::Map::new(
                    PropertyKind::U8,
                    PropertyKind::String,
                    vec![(
                        values::U8::from(1).into(),
                        values::String::from("hello").into(),
                    )],
                )
                .unwrap(),
            )
        },
    );
}

#[test]
fn matrix() {
    assert(
        r#"
    0x1: mtx44 = {
        0.1, 0.2, 0.3, 0.4,
        1.1, 1.2, 1.3, 1.4,
        2.1, 2.2, 2.3, 2.4,
        3.1, 3.2, 3.3, 3.4
    }
    "#,
        |obj| {
            obj.property(
                0x1,
                values::Matrix44::from(glam::Mat4::from_cols_array_2d(&[
                    [0.1, 1.1, 2.1, 3.1],
                    [0.2, 1.2, 2.2, 3.2],
                    [0.3, 1.3, 2.3, 3.3],
                    [0.4, 1.4, 2.4, 3.4],
                ])),
            )
        },
    );
}

#[test]
fn numeric_parse_error() {
    let errs = build_errs("0x1: u8 = 999999");
    assert_eq!(errs.len(), 1, "{errs:#?}");
    assert!(
        matches!(
            errs[0].diagnostic,
            Diagnostic::ParseNumericError {
                expected: PropertyKind::U8,
                ..
            }
        ),
        "{:#?}",
        errs[0]
    );
}

#[test]
fn subtype_count_mismatch_too_many() {
    // Container/list takes exactly 1 subtype
    let errs = build_errs("0x1: list[u8,u8] = {}");
    assert_eq!(errs.len(), 1, "{errs:#?}");
    assert!(
        matches!(
            errs[0].diagnostic,
            Diagnostic::SubtypeCountMismatch {
                expected: 1,
                got: 2,
                ..
            }
        ),
        "{:#?}",
        errs[0]
    );
}

#[test]
fn subtype_count_mismatch_too_few() {
    // Map takes exactly 2 subtypes
    let errs = build_errs("0x1: map[u8] = {}");
    assert_eq!(errs.len(), 1, "{errs:#?}");
    assert!(
        matches!(
            errs[0].diagnostic,
            Diagnostic::SubtypeCountMismatch {
                expected: 2,
                got: 1,
                ..
            }
        ),
        "{:#?}",
        errs[0]
    );
}

#[test]
fn missing_linked_root_entry_reports_diagnostic_without_panicking() {
    let input = r#"
type: string = "PROP"
version: u32 = 3
entries: map[hash,embed] = {}
"#;
    let cst = Cst::parse(input);
    let errs = cst.build_ast(input).diagnostics;
    assert!(
        errs.iter().any(|e| matches!(
            e.diagnostic,
            Diagnostic::MissingRootEntry {
                root_kind: RootKind::Linked
            }
        )),
        "{errs:#?}"
    );
}

#[test]
fn missing_entries_root_entry_reports_diagnostic_without_panicking() {
    let input = r#"
type: string = "PROP"
version: u32 = 3
linked: list[string] = {}
"#;
    let cst = Cst::parse(input);
    let errs = cst.build_ast(input).diagnostics;
    assert!(
        errs.iter().any(|e| matches!(
            e.diagnostic,
            Diagnostic::MissingRootEntry {
                root_kind: RootKind::Entries
            }
        )),
        "{errs:#?}"
    );
}

#[test]
fn missing_type_root_entry_reports_type_not_version() {
    let input = r#"
version: u32 = 3
linked: list[string] = {}
entries: map[hash,embed] = {}
"#;
    let cst = Cst::parse(input);
    let errs = cst.build_ast(input).diagnostics;
    assert!(
        errs.iter().any(|e| matches!(
            e.diagnostic,
            Diagnostic::MissingRootEntry {
                root_kind: RootKind::Type
            }
        )),
        "{errs:#?}"
    );
}

#[test]
fn invalid_type_root_entry_reports_type_not_version() {
    let input = r#"
type: u32 = 3
version: u32 = 3
linked: list[string] = {}
entries: map[hash,embed] = {}
"#;
    let cst = Cst::parse(input);
    let errs = cst.build_ast(input).diagnostics;
    assert!(
        errs.iter().any(|e| matches!(
            e.diagnostic,
            Diagnostic::InvalidRootEntryType {
                root_kind: RootKind::Type,
                ..
            }
        )),
        "{errs:#?}"
    );
}

#[test]
fn empty_vec3_reports_not_enough_items() {
    let errs = build_errs("0x1: vec3 = {}");
    assert_eq!(errs.len(), 1, "{errs:#?}");
    assert!(
        matches!(
            errs[0].diagnostic,
            Diagnostic::NotEnoughItems { got: 0, .. }
        ),
        "{:#?}",
        errs[0]
    );
}

#[test]
fn empty_color_reports_not_enough_items() {
    let errs = build_errs("0x1: rgba = {}");
    assert_eq!(errs.len(), 1, "err count != 1, got: {errs:#?}");
    assert!(
        matches!(
            errs[0].diagnostic,
            Diagnostic::NotEnoughItems { got: 0, .. }
        ),
        "errors dont match, got: {:#?}",
        errs
    );
}

#[test]
fn empty_mtx44_reports_not_enough_items() {
    let errs = build_errs("0x1: mtx44 = {}");
    assert_eq!(errs.len(), 1, "err count != 1, got: {errs:#?}");
    assert!(
        matches!(
            errs[0].diagnostic,
            Diagnostic::NotEnoughItems { got: 0, .. }
        ),
        "errors dont match, got: {:#?}",
        errs
    );
}

/// Asserts `input` produces exactly one diagnostic, and hands it to `is`.
fn assert_one_err<F: Fn(&Diagnostic) -> bool>(input: &str, is: F) -> DiagnosticWithSpan {
    let errs = build_errs(input);
    assert_eq!(errs.len(), 1, "err count != 1, got: {errs:#?}");
    assert!(
        (is)(&errs[0].diagnostic),
        "errors dont match, got: {:#?}",
        errs[0]
    );
    errs[0]
}

/// `pointer`/`embed` values are written `ClassName { .. }`. Deleting the class name
/// used to default-construct a class-hash-0 struct without a word.
#[test]
fn missing_class_name_in_a_list_item_is_reported() {
    let err = assert_one_err(
        r#"
    paramValues: list[embed] = {
        StaticMaterialShaderParamDef {
            name: string = "A"
        }
        {
            name: string = "B"
        }
    }
    "#,
        |d| {
            matches!(
                d,
                Diagnostic::MissingClassName {
                    expected: RitoType {
                        base: PropertyKind::Embedded,
                        ..
                    },
                    ..
                }
            )
        },
    );
    // points at the `{` the class name should precede, not the whole block
    assert_eq!(err.span.len(), 1);
    assert_eq!(
        err.diagnostic.to_string(),
        "Missing class name - embed values are written 'ClassName { .. }'"
    );
}

#[test]
fn missing_class_name_in_a_map_entry_is_reported() {
    assert_one_err(
        r#"
    items: map[hash,pointer] = {
        0xc8fd50ab = {
            name: string = "a"
        }
    }
    "#,
        |d| {
            matches!(
                d,
                Diagnostic::MissingClassName {
                    expected: RitoType {
                        base: PropertyKind::Struct,
                        ..
                    },
                    ..
                }
            )
        },
    );
}

#[test]
fn nested_container_fails() {
    let errs = build_errs(r#"0x1: list[list[u32]] = { { 1 2 } { 3 4 } }"#);
    assert_eq!(errs.len(), 1, "{errs:#?}");
    assert!(
        matches!(errs[0].diagnostic, Diagnostic::InvalidNesting { .. }),
        "{:#?}",
        errs[0]
    );
}

#[test]
fn nested_map_key_type_fails() {
    let errs = build_errs(r#"0x1: map[list,u32] = {}"#);
    assert_eq!(errs.len(), 1, "{errs:#?}");
    assert!(
        matches!(errs[0].diagnostic, Diagnostic::InvalidNesting { .. }),
        "{:#?}",
        errs[0]
    );
}

#[test]
fn nested_optional_type_fails() {
    let errs = build_errs(r#"0x1: option[map] = {}"#);
    assert_eq!(errs.len(), 1, "{errs:#?}");
    assert!(
        matches!(errs[0].diagnostic, Diagnostic::InvalidNesting { .. }),
        "{:#?}",
        errs[0]
    );
}

/// So must one that is properly introduced by a class name.
#[test]
fn a_named_class_list_item_is_fine() {
    let errs = build_errs(
        r#"
    paramValues: list[embed] = {
        StaticMaterialShaderParamDef {
            name: string = "A"
        }
        0xdeadbeef {
            name: string = "B"
        }
    }
    "#,
    );
    assert!(errs.is_empty(), "{errs:#?}");
}

/// The reported case: `""` where a property name belongs. `merge_ir` used to drop any
/// child whose shape didn't fit, with a `trace!` and no diagnostic.
#[test]
fn a_bare_value_in_a_class_body_is_reported() {
    assert_one_err(
        r#"
    name: string = "A"
    ""
    flags: u32 = 1
    "#,
        |d| {
            matches!(
                d,
                Diagnostic::UnexpectedItem {
                    expected: ItemShape::Entry,
                    parent: RitoType {
                        base: PropertyKind::Embedded,
                        ..
                    },
                    ..
                }
            )
        },
    );
}

#[test]
fn a_type_mismatch_blames_the_type_expression_that_set_it() {
    // the LSP renders `expected_span` as "due to this type expression", so it has to point
    // at an actual type expression - not at the container's braces, and not at all when the
    // expectation was not written down anywhere
    let blamed = |input: &str| {
        let text = wrap(input);
        let errs = Cst::parse(&text).build_ast(&text).diagnostics;
        errs.into_iter()
            .find_map(|e| match e.diagnostic {
                Diagnostic::TypeMismatch { expected_span, .. } => Some(expected_span),
                _ => None,
            })
            .expect("expected a TypeMismatch")
            .map(|span| text[span].to_owned())
    };

    assert_eq!(
        blamed(r#"0x1: list[u32] = { "a" }"#).as_deref(),
        Some("list[u32]")
    );
    assert_eq!(
        blamed(r#"0x1: map[u32,u32] = { "5" = 1 }"#).as_deref(),
        Some("map[u32,u32]")
    );
    assert_eq!(
        blamed(r#"0x1: option[u32] = { "a" }"#).as_deref(),
        Some("option[u32]")
    );
    // a numeric literal resolved against a hint that takes no number
    assert_eq!(blamed(r#"0x1: string = 5"#).as_deref(), Some("string"));
    // listlike components answer to the type that made them components
    assert_eq!(
        blamed(r#"0x1: vec3 = { 1, "a", 3 }"#).as_deref(),
        Some("vec3")
    );
    assert_eq!(
        blamed(r#"0x1: rgba = { 1, "a", 3, 4 }"#).as_deref(),
        Some("rgba")
    );
    // ... and a listlike written as a list item falls back to the container's subtype
    assert_eq!(
        blamed(r#"0x1: list[vec3] = { { 1, "a", 3 } }"#).as_deref(),
        Some("list[vec3]")
    );
    // a property name is a hash because it is a property name - no type expression said so
    assert_eq!(blamed("true: u32 = 3").as_deref(), None);
}

#[test]
fn a_wrong_shaped_item_is_underlined_whole() {
    // a parent rejects the item, not a part of it - from the list's point of view the whole
    // 'key: u32 = 1' is the mistake, even though '1' on its own would be a fine list item
    let underlined = |input: &str| {
        let err = assert_one_err(input, |d| matches!(d, Diagnostic::UnexpectedItem { .. }));
        wrap(input)[err.span].to_owned()
    };

    assert_eq!(
        underlined(r#"0x1: list[u32] = { key: u32 = 1 }"#),
        "key: u32 = 1"
    );
    assert_eq!(
        underlined(r#"0x1: list[u32] = { 0xdead = 1 }"#),
        "0xdead = 1"
    );
    assert_eq!(
        underlined(r#"0x1: list[u32] = { "key" = 1 }"#),
        r#""key" = 1"#
    );
    assert_eq!(
        underlined(r#"0x1: option[u32] = { key: u32 = 1 }"#),
        "key: u32 = 1"
    );
    assert_eq!(underlined(r#"0x1: map[hash,u32] = { 5 }"#), "5");
}

#[test]
fn an_unexpected_item_names_the_shape_its_parent_wants() {
    let is_shape = |d: &Diagnostic| matches!(d, Diagnostic::UnexpectedItem { .. });

    let map = assert_one_err(r#"0x1: map[hash,u32] = { 5 }"#, is_shape);
    assert_eq!(
        map.diagnostic.to_string(),
        "map[hash,u32] takes an entry ('name: type = value')"
    );

    let class = assert_one_err(
        r#"
    name: string = "A"
    ""
    flags: u32 = 1
    "#,
        is_shape,
    );
    assert_eq!(
        class.diagnostic.to_string(),
        "embed takes an entry ('name: type = value')"
    );

    let list = assert_one_err(r#"0x1: list[u32] = { key: u32 = 1 }"#, is_shape);
    assert_eq!(list.diagnostic.to_string(), "list[u32] takes a value");
}

/// A map entry takes its value type from the map's subtype, but writing it out is allowed -
/// it just has to agree with what the map declared.
#[test]
fn a_map_entry_may_declare_its_value_type() {
    assert(r#"0x1: map[hash,u32] = { 0xdead: u32 = 1 }"#, |obj| {
        obj.property(
            0x1,
            values::Map::new(
                PropertyKind::Hash,
                PropertyKind::U32,
                vec![(
                    values::Hash::from(BinHash::from(0xdeadu32)).into(),
                    values::U32::from(1u32).into(),
                )],
            )
            .unwrap(),
        )
    });

    let err = assert_one_err(r#"0x1: map[hash,u32] = { 0xdead: string = "a" }"#, |d| {
        matches!(d, Diagnostic::TypeMismatch { .. })
    });
    assert_eq!(
        err.diagnostic.to_string(),
        "Type mismatch - expected u32, got string"
    );
}

#[test]
fn an_entry_in_a_list_is_reported() {
    assert_one_err(r#"0x1: list[u32] = { key: u32 = 1 }"#, |d| {
        matches!(
            d,
            Diagnostic::UnexpectedItem {
                expected: ItemShape::Value,
                parent: RitoType {
                    base: PropertyKind::Container,
                    ..
                },
                ..
            }
        )
    });
}

#[test]
fn a_bare_value_in_a_map_is_reported() {
    assert_one_err(r#"0x1: map[hash,u32] = { 5 }"#, |d| {
        matches!(
            d,
            Diagnostic::UnexpectedItem {
                expected: ItemShape::Entry,
                parent: RitoType {
                    base: PropertyKind::Map,
                    ..
                },
                ..
            }
        )
    });
}

#[test]
fn an_entry_in_an_option_is_reported() {
    assert_one_err(r#"0x1: option[u32] = { key: u32 = 1 }"#, |d| {
        matches!(
            d,
            Diagnostic::UnexpectedItem {
                expected: ItemShape::Value,
                parent: RitoType {
                    base: PropertyKind::Optional,
                    ..
                },
                ..
            }
        )
    });
}

/// A key that can't become a hash was dropped silently by `merge_ir`.
#[test]
fn a_property_name_that_cannot_be_hashed_is_reported() {
    let err = assert_one_err(r#"true: u32 = 3"#, |d| {
        matches!(
            d,
            Diagnostic::TypeMismatch {
                expected: RitoType {
                    base: PropertyKind::Hash,
                    ..
                },
                ..
            }
        )
    });
    assert_eq!(
        err.diagnostic.to_string(),
        "Type mismatch - expected hash, got bool"
    );
}

#[test]
fn a_quoted_property_name_produces_the_same_property_as_a_bare_one() {
    let quoted = wrap(r#""skinClassification": u32 = 1"#);
    let bare = wrap(r#"skinClassification: u32 = 1"#);

    let quoted_ast = Cst::parse(&quoted).build_ast(&quoted);
    let bare_ast = Cst::parse(&bare).build_ast(&bare);

    assert_eq!(
        quoted_ast.diagnostics.len(),
        1,
        "{:#?}",
        quoted_ast.diagnostics
    );
    assert!(
        bare_ast.diagnostics.is_empty(),
        "{:#?}",
        bare_ast.diagnostics
    );
    pretty_assertions::assert_eq!(quoted_ast.to_bin(&quoted), bare_ast.to_bin(&bare));
}

/// Quoted property names are hashed as written - `""` becomes `hash("")`.
#[test]
fn a_quoted_property_name_is_reported() {
    let err = assert_one_err(r#""": u32 = 1"#, |d| {
        matches!(
            d,
            Diagnostic::QuotedPropertyName {
                parent: RitoType {
                    base: PropertyKind::Embedded,
                    ..
                },
                ..
            }
        )
    });
    assert_eq!(
        err.diagnostic.to_string(),
        "Quoted property name - embed bodies take 'name: type = value', with the name \
         unquoted or a '0x..' hash"
    );
}

#[test]
fn map_keys_of_every_key_type_keep_their_pair() {
    for (ty, key) in [
        ("hash", r#"0x1"#),
        ("hash", r#""Characters/Aatrox/Skins/Skin0""#),
        ("string", r#""Characters/Aatrox/Skins/Skin0""#),
        ("link", r#""Characters/Aatrox/Skins/Skin0""#),
        ("file", r#""ASSETS/Maps/Textures/Bloom.tex""#),
        ("file", r#"0x1"#),
        ("u8", "1"),
        ("u16", "1"),
        ("u32", "1"),
        ("u64", "1"),
        ("i8", "-1"),
        ("i16", "-1"),
        ("i32", "-1"),
        ("i64", "-1"),
        ("f32", "1.5"),
    ] {
        let input = wrap(&format!("0x1: map[{ty},u32] = {{ {key} = 1 }}"));
        let ast = Cst::parse(&input).build_ast(&input);
        assert!(
            ast.diagnostics.is_empty(),
            "map[{ty},u32] with key {key}: {:#?}",
            ast.diagnostics
        );
        let bin = ast.to_bin(&input);

        // No diagnostics is not the same as no data lost - that is the whole failure
        // mode this change is about, so check the pair actually reached the bin.
        let PropertyValueEnum::Map(map) =
            &bin.objects[&BinHash(0xDEADBEEF)].properties[&BinHash(1)]
        else {
            panic!("map[{ty},u32] with key {key}: property is not a map");
        };
        assert_eq!(
            map.entries().len(),
            1,
            "map[{ty},u32] with key {key}: pair was dropped"
        );
    }
}

/// Numeric map keys are written and read bare. Upstream's `read_word` accepts only
/// `[A-Za-z0-9_+-.]`, so on `"7"` it stops at the quote and hands `read_number` an empty
/// word, which fails - see
/// <https://github.com/moonshadow565/ritobin/blob/d4b8764939d141c1db3ffd186d49bf60fd889b87/ritobin_lib/src/ritobin/bin_io_text_read.cpp#L69-L81>.
///
/// The syntax is rejected rather than parsed out of the quotes, so a key whose contents
/// would have been a perfectly good number (`"7"`) is reported just the same as one that
/// could never be (`"abc"`).
#[test]
fn a_quoted_numeric_map_key_is_reported() {
    for ty in ["u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64", "f32"] {
        for key in ["7", "0.5", "abc", ""] {
            let errs = build_errs(&format!(r#"0x1: map[{ty},u32] = {{ "{key}" = 1 }}"#));
            assert_eq!(errs.len(), 1, "map[{ty},u32] key {key:?}: {errs:#?}");
            assert_eq!(
                errs[0].diagnostic.to_string(),
                format!("Type mismatch - expected {ty}, got string"),
                "map[{ty},u32] key {key:?}"
            );
        }
    }
}

/// Reporting a rejected key does not recover the pair - it is still missing from the
/// bin. The diagnostic is the whole of the fix here.
#[test]
fn a_rejected_map_key_still_drops_the_pair() {
    let input = wrap(r#"0x1: map[u32,u32] = { "0.5" = 1 }"#);
    let ast = Cst::parse(&input).build_ast(&input);

    assert_eq!(ast.diagnostics.len(), 1, "{:#?}", ast.diagnostics);
    pretty_assertions::assert_eq!(
        ast.to_bin(&input),
        Bin::builder()
            .object(
                BinObject::<NoMeta>::builder(0xDEADBEEF, 0x1234123)
                    .property(
                        0x1,
                        values::Map::new(PropertyKind::U32, PropertyKind::U32, vec![]).unwrap()
                    )
                    .build()
            )
            .build()
    );
}

/// The root `entries` map is the one every file has - its keys are quoted paths.
#[test]
fn a_quoted_root_entry_key_is_fine() {
    let input = r#"
type: string = "PROP"
version: u32 = 3
linked: list[string] = {}
entries: map[hash,embed] = {
    "Characters/Aatrox/Skins/Skin0" = 0x1234123 {
        0x1: u32 = 1
    }
}
"#;
    let cst = Cst::parse(input);
    let errs = cst.build_ast(input).diagnostics;
    assert!(errs.is_empty(), "{errs:#?}");
}
