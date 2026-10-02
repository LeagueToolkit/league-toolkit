//! C++ ritobin's text is the canonical form: a bin prints as `ritobin_cli -k -i bin -o text`
//! prints it, and that text parses back to the same bin.
//!
//! The fixtures in `tests/data` are synthetic. Each `.rito` is `ritobin_cli`'s text for the `.bin`
//! beside it, from ritobin `368b413`.

use std::io::Cursor;

use glam::{Mat4, Vec4};
use ltk_hash::{BinHash, Hash as _};
use ltk_meta::{property::values, Bin, BinObject, PropertyKind, PropertyValueEnum};
use ltk_ritobin::{ast::diagnostics::Diagnostic, Cst, PrintCanonical as _};

fn fixture(name: &str) -> (Vec<u8>, String) {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/");
    let bin = std::fs::read(format!("{dir}{name}.bin")).unwrap();
    let text = std::fs::read_to_string(format!("{dir}{name}.rito")).unwrap();
    (bin, text)
}

fn read(bytes: &[u8]) -> Bin {
    Bin::from_reader(&mut Cursor::new(bytes)).unwrap()
}

fn write(bin: &Bin) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    bin.to_writer(&mut out).unwrap();
    out.into_inner()
}

/// Parses and builds `text`, asserting it has no parse error and no diagnostic.
fn build(text: &str) -> Bin {
    let cst = Cst::parse(text);
    assert!(cst.errors.is_empty(), "parse errors: {:#?}", cst.errors);
    let partial = cst.build_bin(text);
    assert!(
        partial.diagnostics.is_empty(),
        "diagnostics: {:#?}",
        partial.diagnostics
    );
    partial.bin
}

/// A `PROP` file whose one object, `0x00000001 = 0x00000002`, holds `fields`, each line at the
/// object's field indent.
fn prop(fields: &str) -> String {
    let fields: String = fields.lines().map(|l| format!("        {l}\n")).collect();
    format!(
        "#PROP_text\ntype: string = \"PROP\"\nversion: u32 = 3\nlinked: list[string] = {{}}\n\
         entries: map[hash,embed] = {{\n    0x00000001 = 0x00000002 {{\n{fields}    }}\n}}\n"
    )
}

/// The bin [`prop`] describes, with `fields` as its object's properties.
fn prop_bin(fields: impl IntoIterator<Item = (u32, PropertyValueEnum)>) -> Bin {
    let object = fields
        .into_iter()
        .fold(BinObject::builder(1, 2), |o, (name, value)| {
            o.property(name, value)
        });
    Bin::builder().object(object.build()).build()
}

/// The properties of [`prop`]'s one object.
fn fields(bin: &Bin) -> Vec<(u32, PropertyValueEnum)> {
    bin.objects[&BinHash(1)]
        .properties
        .iter()
        .map(|(name, value)| (name.0, value.clone()))
        .collect()
}

fn null_pointer() -> PropertyValueEnum {
    values::Struct {
        class_hash: BinHash(0),
        properties: Default::default(),
    }
    .into()
}

fn f32(v: f32) -> PropertyValueEnum {
    values::F32::new(v).into()
}

fn string(s: &str) -> PropertyValueEnum {
    values::String::from(s).into()
}

// -- the fixtures ---------------------------------------------------------------------------------

#[test]
fn test_bin_prints_as_cpp_ritobin_text() {
    let (bin, text) = fixture("test");
    pretty_assertions::assert_eq!(read(&bin).print_canonical().unwrap(), text);
}

#[test]
fn cpp_ritobin_text_parses_to_test_bin() {
    let (bytes, text) = fixture("test");
    let bin = build(&text);
    assert_eq!(bin, read(&bytes));
    assert_eq!(write(&bin), bytes);
}

#[test]
fn edge_prop_bin_prints_as_cpp_ritobin_text() {
    let (bin, text) = fixture("edge-prop");
    pretty_assertions::assert_eq!(read(&bin).print_canonical().unwrap(), text);
}

#[test]
fn cpp_ritobin_text_parses_to_edge_prop_bin() {
    let (bytes, text) = fixture("edge-prop");
    // The fixture holds a NaN, unequal to itself: the written bytes are compared, not the bins.
    assert_eq!(write(&build(&text)), bytes);
}

#[test]
fn version_1_bin_prints_without_linked_root() {
    let (bin, text) = fixture("edge-v1");
    assert!(!text.contains("linked:"));
    pretty_assertions::assert_eq!(read(&bin).print_canonical().unwrap(), text);
}

#[test]
fn version_1_text_without_linked_root_parses_to_its_bin() {
    let (bytes, text) = fixture("edge-v1");
    let bin = build(&text);
    assert_eq!(bin.version, 1);
    assert_eq!(bin, read(&bytes));
}

// -- parsing --------------------------------------------------------------------------------------

#[test]
fn scientific_float_keeps_its_exponent() {
    let bin = build(&prop(
        "0x00000010: f32 = 1e+05\n0x00000011: f32 = -8.742278e-08\n0x00000012: f32 = 1e-45",
    ));
    assert_eq!(
        fields(&bin),
        [
            (0x10, f32(1e5)),
            (0x11, f32(-8.742278e-8)),
            (0x12, f32(f32::from_bits(1)))
        ]
    );
}

#[test]
fn vec4_with_scientific_float_keeps_its_values() {
    let bin = build(&prop(
        "0x00000015: vec4 = { 2.315781e-05, 0, 0.14117648, 0 }",
    ));
    assert_eq!(
        fields(&bin),
        [(
            0x15,
            values::Vector4::new(Vec4::new(2.315781e-5, 0.0, 0.14117648, 0.0)).into()
        )]
    );
}

#[test]
fn non_finite_floats_parse() {
    let bin = build(&prop(
        "0x00000001: f32 = inf\n0x00000002: f32 = -inf\n0x00000003: vec2 = { nan, -0 }",
    ));
    let [(_, PropertyValueEnum::F32(inf)), (_, PropertyValueEnum::F32(neg_inf)), (_, PropertyValueEnum::Vector2(v))] =
        &fields(&bin)[..]
    else {
        panic!("{bin:#?}");
    };
    assert_eq!(**inf, f32::INFINITY);
    assert_eq!(**neg_inf, f32::NEG_INFINITY);
    assert!(v.x.is_nan());
    assert!(v.y == 0.0 && v.y.is_sign_negative());
}

#[test]
fn non_finite_floats_parse_as_f32_map_keys() {
    let bin = build(&prop(
        "0x00000001: map[f32,u32] = {\n    inf = 1\n    -inf = 2\n}",
    ));
    let [(_, PropertyValueEnum::Map(map))] = &fields(&bin)[..] else {
        panic!("{bin:#?}");
    };
    let keys: Vec<_> = map
        .entries()
        .iter()
        .map(|(k, _)| match k {
            PropertyValueEnum::F32(k) => **k,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(keys, [f32::INFINITY, f32::NEG_INFINITY]);
}

#[test]
fn fields_and_classes_named_inf_or_nan_are_names() {
    let bin = build(&prop(
        "inf: f32 = 1\nnan: embed = inf {\n    nan: u32 = 2\n}",
    ));
    let hash = |name: &str| BinHash::hash_str(name).0;
    let embed = values::Embedded(values::Struct {
        class_hash: BinHash::hash_str("inf"),
        properties: [(BinHash::hash_str("nan"), values::U32::new(2).into())]
            .into_iter()
            .collect(),
    });
    assert_eq!(
        fields(&bin),
        [(hash("inf"), f32(1.0)), (hash("nan"), embed.into())]
    );
}

#[test]
fn multi_line_mtx44_reads_row_by_row() {
    let bin = build(&prop(
        "0x00000016: mtx44 = {
    1, 2, 3, 4
    5, 6, 7, 8
    9, 10, 11, 12
    13, 14, 15, 1e+05
}",
    ));
    let rows = [
        1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 1e5,
    ];
    assert_eq!(
        fields(&bin),
        [(
            0x16,
            values::Matrix44::new(Mat4::from_cols_array(&rows).transpose()).into()
        )]
    );
}

#[test]
fn null_pointer_parses_and_keeps_later_fields() {
    let bin = build(&prop(
        "0x00000020: pointer = null
0x00000021: list[pointer] = {
    null
    0x00000030 {}
}
0x00000099: u32 = 7",
    ));
    let pointer = values::Struct {
        class_hash: BinHash(0x30),
        properties: Default::default(),
    };
    assert_eq!(
        fields(&bin),
        [
            (0x20, null_pointer()),
            (
                0x21,
                values::Container::new(PropertyKind::Struct, vec![null_pointer(), pointer.into()])
                    .unwrap()
                    .into()
            ),
            (0x99, values::U32::new(7).into()),
        ]
    );
}

#[test]
fn null_is_not_a_value_of_another_type() {
    let text = prop("0x00000001: u32 = null");
    let partial = Cst::parse(&text).build_bin(&text);
    assert!(
        matches!(
            partial.diagnostics[..],
            [ref d] if matches!(d.diagnostic, Diagnostic::TypeMismatch { .. })
        ),
        "{:#?}",
        partial.diagnostics
    );
}

#[test]
fn string_ending_in_escaped_backslash_terminates() {
    let bin = build(&prop(
        r#"0x00000040: string = "ConstantShadow\\"
0x00000051: map[hash,string] = {
    0x000000aa = "end\\"
}"#,
    ));
    assert_eq!(
        fields(&bin),
        [
            (0x40, string("ConstantShadow\\")),
            (
                0x51,
                values::Map::new(
                    PropertyKind::Hash,
                    PropertyKind::String,
                    vec![(values::Hash::new(0xaa).into(), string("end\\"))],
                )
                .unwrap()
                .into()
            ),
        ]
    );
}

#[test]
fn map_keys_take_vectors_and_64_bit_hashes() {
    let text = prop(
        "0x00000001: map[vec3,rgba] = {
    { 1e+05, 0, -1 } = { 1, 2, 3, 4 }
}
0x00000002: map[file,link] = {
    0xffffffffffffffff = 0x00000000
}",
    );
    let bin = build(&text);
    assert_eq!(fields(&bin).len(), 2);
    assert_eq!(bin.print_canonical().unwrap(), text);
}

// -- printing -------------------------------------------------------------------------------------

#[test]
fn strings_keep_spaces_at_their_ends() {
    let bin = prop_bin([(0x43, string(" a ")), (0x44, string("pad "))]);
    assert_eq!(
        bin.print_canonical().unwrap(),
        prop("0x00000043: string = \" a \"\n0x00000044: string = \"pad \"")
    );
}

#[test]
fn strings_print_with_cpp_escapes() {
    let bin = prop_bin([(0x44, string("tab\there \"quoted\" back\\slash\x01 é"))]);
    assert_eq!(
        bin.print_canonical().unwrap(),
        prop(r#"0x00000044: string = "tab\there \"quoted\" back\\slash\x01 é""#)
    );
}

#[test]
fn null_pointer_prints_as_null() {
    let bin = prop_bin([(0x20, null_pointer())]);
    assert_eq!(
        bin.print_canonical().unwrap(),
        prop("0x00000020: pointer = null")
    );
}

#[test]
fn mtx44_prints_four_rows_of_four() {
    let rows = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 2.5, 1e5, 0.0, 1.0,
    ];
    let bin = prop_bin([(
        0x16,
        values::Matrix44::new(Mat4::from_cols_array(&rows).transpose()).into(),
    )]);
    assert_eq!(
        bin.print_canonical().unwrap(),
        prop(
            "0x00000016: mtx44 = {
    1, 0, 0, 0
    0, 1, 0, 0
    0, 0, 1, 0
    2.5, 1e+05, 0, 1
}"
        )
    );
}

#[test]
fn list_of_pointers_closes_at_its_field_indent() {
    let pointer = values::Struct {
        class_hash: BinHash(0x30),
        properties: [(BinHash(0x31), values::U32::new(1).into())]
            .into_iter()
            .collect(),
    };
    let list =
        values::Container::new(PropertyKind::Struct, vec![null_pointer(), pointer.into()]).unwrap();
    let bin = prop_bin([(0x21, list.into())]);
    assert_eq!(
        bin.print_canonical().unwrap(),
        prop(
            "0x00000021: list[pointer] = {
    null
    0x00000030 {
        0x00000031: u32 = 1
    }
}"
        )
    );
}

#[test]
fn floats_print_in_shortest_to_chars_form() {
    let bin = prop_bin([
        (0x10, f32(1e5)),
        (0x11, f32(2.315781e-5)),
        (0x12, f32(0.14117648)),
        (0x13, f32(4294967296.0)),
        (0x14, f32(-28936.8125)),
    ]);
    assert_eq!(
        bin.print_canonical().unwrap(),
        prop(
            "0x00000010: f32 = 1e+05
0x00000011: f32 = 2.315781e-05
0x00000012: f32 = 0.14117648
0x00000013: f32 = 4294967296
0x00000014: f32 = -28936.812"
        )
    );
}

#[test]
fn empty_containers_print_as_empty_braces() {
    let bin = prop_bin([
        (
            0x1,
            values::Container::empty(PropertyKind::U32).unwrap().into(),
        ),
        (
            0x2,
            values::Map::empty(PropertyKind::Hash, PropertyKind::Embedded)
                .unwrap()
                .into(),
        ),
        (
            0x3,
            values::Embedded(values::Struct {
                class_hash: BinHash(0x40),
                properties: Default::default(),
            })
            .into(),
        ),
    ]);
    assert_eq!(
        bin.print_canonical().unwrap(),
        prop(
            "0x00000001: list[u32] = {}
0x00000002: map[hash,embed] = {}
0x00000003: embed = 0x00000040 {}"
        )
    );
}
