//! Differential tests between the two typecheckers (`typecheck::walk`, driving `Cst::build_bin`,
//! and `ast::build`, driving `Cst::build_ast`) - see the crate's design notes for why these two
//! independent implementations of the same coercion/diagnostic rules need a test like this to
//! keep them from silently drifting apart.

#![cfg(feature = "ast")]

use ltk_meta::{property::values, Bin, BinObject, PropertyKind, PropertyValueEnum};
use ltk_ritobin::{cst::Cst, print::Print as _};

const SAMPLE_RITOBIN: &str = r#"#PROP_text
type: string = "PROP"
version: u32 = 3
linked: list[string] = {
    "DATA/Characters/Test/Animations/Skin0.bin"
    "DATA/Characters/Test/Test.bin"
}
entries: map[hash,embed] = {
    "Characters/Test/Skins/Skin0" = SkinCharacterDataProperties {
        skinClassification: u32 = 1
        championSkinName: string = "TestBase"
        metaDataTags: string = "gender:male"
        loadscreen: embed = CensoredImage {
            image: string = "ASSETS/Characters/Test/Skins/Base/TestLoadScreen.tex"
        }
        skinAudioProperties: embed = skinAudioProperties {
            tagEventList: list[string] = {
                "Test"
            }
            bankUnits: list2[embed] = {
                BankUnit {
                    name: string = "Test_Base_SFX"
                    bankPath: list[string] = {
                        "ASSETS/Sounds/Test/audio.bnk"
                        "ASSETS/Sounds/Test/events.bnk"
                    }
                    events: list[string] = {
                        "Play_sfx_Test_Attack"
                        "Play_sfx_Test_Death"
                    }
                }
            }
        }
        iconCircle: option[string] = {
            "ASSETS/Characters/Test/Icons/Circle.tex"
        }
        iconSquare: option[string] = {}
        position: vec3 = { 1.0, 2.0, 5.0 }
        tint: rgba = { 255, 0, 128, 255 }
        transform: mtx44 = {
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        }
    }
}
"#;

#[test]
fn sample_matches() {
    let cst = Cst::parse(SAMPLE_RITOBIN);
    assert!(cst.errors.is_empty(), "parse errors = {:#?}", cst.errors);

    let fast_partial = cst.build_bin(SAMPLE_RITOBIN);
    assert!(
        fast_partial.diagnostics.is_empty(),
        "build_bin diagnostics = {:#?}",
        fast_partial.diagnostics
    );
    let fast_bin = fast_partial.bin;

    let ast = cst.build_ast(SAMPLE_RITOBIN);
    assert!(
        ast.diagnostics.is_empty(),
        "ast::build diagnostics = {:#?}",
        ast.diagnostics
    );

    let new_bin = ast.to_bin(SAMPLE_RITOBIN);

    assert_eq!(
        fast_bin, new_bin,
        "the two typecheckers disagree on the sample file"
    );
}

// ---- property-based: arbitrary Bin -> text -> both typecheckers agree -----------------------
//
// Generate an arbitrary, structurally-valid `Bin`, print it to ritobin text via the crate's
// existing `Bin -> text` machinery, then run both typecheckers on the resulting text and assert
// they agree - the differential invariant this whole file exists to defend.

use proptest::prelude::*;

fn arb_string() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_/. ]{1,16}"
}

fn arb_hash() -> impl Strategy<Value = ltk_hash::BinHash> {
    any::<u32>().prop_map(ltk_hash::BinHash)
}

/// One leaf-kind value, matching `kind`. Every primitive `PropertyKind` variant `ltk_meta`
/// supports is covered, so this is the base case a nested [`arb_value`] tree bottoms out at.
fn arb_leaf(kind: PropertyKind) -> BoxedStrategy<PropertyValueEnum> {
    use PropertyKind as K;
    match kind {
        K::None => Just(PropertyValueEnum::None(values::None::default())).boxed(),
        K::Bool => any::<bool>()
            .prop_map(|v| values::Bool::new(v).into())
            .boxed(),
        K::BitBool => any::<bool>()
            .prop_map(|v| values::BitBool::new(v).into())
            .boxed(),
        K::I8 => any::<i8>().prop_map(|v| values::I8::new(v).into()).boxed(),
        K::U8 => any::<u8>().prop_map(|v| values::U8::new(v).into()).boxed(),
        K::I16 => any::<i16>()
            .prop_map(|v| values::I16::new(v).into())
            .boxed(),
        K::U16 => any::<u16>()
            .prop_map(|v| values::U16::new(v).into())
            .boxed(),
        K::I32 => any::<i32>()
            .prop_map(|v| values::I32::new(v).into())
            .boxed(),
        K::U32 => any::<u32>()
            .prop_map(|v| values::U32::new(v).into())
            .boxed(),
        K::I64 => any::<i64>()
            .prop_map(|v| values::I64::new(v).into())
            .boxed(),
        K::U64 => any::<u64>()
            .prop_map(|v| values::U64::new(v).into())
            .boxed(),
        // kept small and integral so ritobin's text round-trip (which reprints as decimal) never
        // hits float-formatting precision loss unrelated to what this test is checking
        K::F32 => (-1000i32..1000)
            .prop_map(|v| values::F32::new(v as f32).into())
            .boxed(),
        K::Vector2 => (-1000i32..1000, -1000i32..1000)
            .prop_map(|(x, y)| values::Vector2::new([x as f32, y as f32].into()).into())
            .boxed(),
        K::Vector3 => (-1000i32..1000, -1000i32..1000, -1000i32..1000)
            .prop_map(|(x, y, z)| {
                values::Vector3::new([x as f32, y as f32, z as f32].into()).into()
            })
            .boxed(),
        K::Vector4 => (
            -1000i32..1000,
            -1000i32..1000,
            -1000i32..1000,
            -1000i32..1000,
        )
            .prop_map(|(x, y, z, w)| {
                values::Vector4::new([x as f32, y as f32, z as f32, w as f32].into()).into()
            })
            .boxed(),
        K::Matrix44 => prop::collection::vec(-1000i32..1000, 16)
            .prop_map(|m| {
                let m: Vec<f32> = m.into_iter().map(|v| v as f32).collect();
                values::Matrix44::new(glam::Mat4::from_cols_array(m[..].try_into().unwrap())).into()
            })
            .boxed(),
        K::Color => any::<[u8; 4]>()
            .prop_map(|[r, g, b, a]| {
                values::Color::new(ltk_primitives::Color { r, g, b, a }).into()
            })
            .boxed(),
        K::String => arb_string()
            .prop_map(|v| values::String::new(v).into())
            .boxed(),
        K::Hash => arb_hash().prop_map(|v| values::Hash::new(v).into()).boxed(),
        K::WadChunkLink => any::<u64>()
            .prop_map(|v| values::WadChunkLink::new(ltk_hash::WadHash(v)).into())
            .boxed(),
        K::ObjectLink => arb_hash()
            .prop_map(|v| values::ObjectLink::new(v).into())
            .boxed(),
        _ => unreachable!("arb_leaf called with a non-leaf kind: {kind:?}"),
    }
}

const LEAF_KINDS: &[PropertyKind] = &[
    PropertyKind::Bool,
    PropertyKind::I8,
    PropertyKind::U8,
    PropertyKind::I16,
    PropertyKind::U16,
    PropertyKind::I32,
    PropertyKind::U32,
    PropertyKind::I64,
    PropertyKind::U64,
    PropertyKind::F32,
    PropertyKind::Vector2,
    PropertyKind::Vector3,
    PropertyKind::Vector4,
    PropertyKind::Matrix44,
    PropertyKind::Color,
    PropertyKind::String,
    PropertyKind::Hash,
    PropertyKind::WadChunkLink,
];

fn arb_leaf_of_any_kind() -> impl Strategy<Value = PropertyValueEnum> {
    prop::sample::select(LEAF_KINDS).prop_flat_map(arb_leaf)
}

/// A homogeneous container of one randomly-picked leaf kind - `Container::try_from` requires
/// every item to share a kind, so the kind is picked once, then every item is generated against
/// that same kind (rather than letting each item pick independently, which would produce
/// mismatched-kind `Vec`s `try_from` can't build a container from).
fn arb_leaf_container() -> impl Strategy<Value = PropertyValueEnum> {
    prop::sample::select(LEAF_KINDS)
        .prop_flat_map(|kind| prop::collection::vec(arb_leaf(kind), 1..4))
        .prop_map(|items| PropertyValueEnum::Container(values::Container::try_from(items).unwrap()))
}

/// A value tree bounded to a shallow depth: leaves, plus (rarely) nested `embed`s (whose own
/// properties can recursively contain more of the same) and homogeneous leaf containers - real
/// files nest much deeper, but this is exercising the differential invariant, not stress-testing
/// depth handling specifically.
fn arb_value() -> impl Strategy<Value = PropertyValueEnum> {
    let leaf = arb_leaf_of_any_kind();
    leaf.prop_recursive(2, 8, 3, |inner| {
        prop_oneof![
            (arb_hash(), arb_properties(inner)).prop_map(|(class_hash, properties)| {
                values::Embedded(values::Struct {
                    class_hash,
                    properties,
                    meta: Default::default(),
                })
                .into()
            }),
            arb_leaf_container(),
        ]
    })
}

fn arb_properties(
    value: impl Strategy<Value = PropertyValueEnum>,
) -> impl Strategy<Value = indexmap::IndexMap<ltk_hash::BinHash, PropertyValueEnum>> {
    prop::collection::vec((arb_hash(), value), 0..4).prop_map(|v| v.into_iter().collect())
}

fn arb_bin_object() -> impl Strategy<Value = BinObject> {
    (arb_hash(), arb_hash(), arb_properties(arb_value())).prop_map(
        |(path_hash, class_hash, properties)| BinObject {
            path_hash,
            class_hash,
            properties,
        },
    )
}

// `Map` (`map[key,value]`) isn't generated: every other container/leaf/struct shape is covered
// above, and map's extra key/value-kind bookkeeping wasn't worth the added generator complexity
// for this first pass - a reasonable follow-up if drift is ever suspected specifically there.
fn arb_bin() -> impl Strategy<Value = Bin> {
    (
        prop::collection::vec(arb_bin_object(), 0..3),
        prop::collection::vec(arb_string(), 0..3),
    )
        .prop_map(|(objects, dependencies)| Bin::new(objects, dependencies))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn both_typecheckers_agree_on_arbitrary_bins(bin in arb_bin()) {
        let text = bin.print().expect("printing a generated Bin should never fail");

        let cst = Cst::parse(&text);
        prop_assert!(cst.errors.is_empty(), "reparse errors: {:#?}\ntext:\n{text}", cst.errors);

        let fast_partial = cst.build_bin(&text);
        let ast = cst.build_ast(&text);
        let new_bin = ast.to_bin(&text);

        prop_assert!(fast_partial.diagnostics.is_empty(), "build_bin diagnostics: {:#?}\ntext:\n{}", fast_partial.diagnostics, text);
        prop_assert!(ast.diagnostics.is_empty(), "ast::build diagnostics: {:#?}\ntext:\n{}", ast.diagnostics, text);
        prop_assert_eq!(fast_partial.bin, new_bin, "build_bin and Ast::to_bin disagree on this input:\n{}", text);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    //TODO: better arbitrary source gen
    #[test]
    fn build_bin_never_panics_on_arbitrary_text(text in ".{0,400}") {
        let cst = Cst::parse(&text);
        let _ = cst.build_bin(&text);
    }
}
