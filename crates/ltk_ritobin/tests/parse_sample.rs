//! Integration test for parsing a sample ritobin file.

use ltk_ritobin::{parse::parse, print::Print as _};

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
    }
}
"#;

fn tree(input: &str) -> String {
    let cst = parse(input);
    assert!(cst.errors.is_empty());

    let mut debug = String::new();
    cst.print(&mut debug, 0, input);

    // debug);
    debug
}

#[test]
fn test_roundtrip() {
    let cst = parse(SAMPLE_RITOBIN);
    let (tree, errors) = cst.build_bin(SAMPLE_RITOBIN);
    assert!(errors.is_empty(), "errors = {errors:#?}");

    // Write back to text
    let output = tree.print().expect("Failed to write");

    println!("output:\n{output}");

    // Parse again
    let cst2 = parse(&output);
    assert!(
        cst2.errors.is_empty(),
        "reparse errors = {:#?}",
        cst2.errors
    );

    let mut str = String::new();
    cst2.print(&mut str, 0, &output);
    println!("reparsed:\n{str}");

    let (tree2, errors) = cst2.build_bin(&output);
    assert!(errors.is_empty(), "build bin errors = {errors:#?}");

    // Verify structure is preserved
    assert_eq!(tree.version, tree2.version);
    assert_eq!(tree.dependencies.len(), tree2.dependencies.len());
    assert_eq!(tree.objects.len(), tree2.objects.len());
}
#[test]
fn test_parse_primitives() {
    insta::assert_snapshot!(tree(
        r#"
test_bool: bool = true
test_i8: i8 = -128
test_u8: u8 = 255
test_i16: i16 = -32768
test_u16: u16 = 65535
test_i32: i32 = -2147483648
test_u32: u32 = 4294967295
test_f32: f32 = 3.14159
test_vec2: vec2 = { 1.0, 2.0 }
test_vec3: vec3 = { 1.0, 2.0, 3.0 }
test_vec4: vec4 = { 1.0, 2.0, 3.0, 4.0 }
test_rgba: rgba = { 255, 128, 64, 255 }
test_string: string = "Hello, World!"
test_hash: hash = 0xdeadbeef
test_link: link = "path/to/object"
test_flag: flag = false
"#,
    ));
}

#[test]
fn test_parse_containers() {
    insta::assert_snapshot!(tree(
        r#"
test_list: list[string] = {
    "item1"
    "item2"
    "item3"
}
test_list2: list2[u32] = {
    1
    2
    3
}
test_option_some: option[string] = {
    "value"
}
test_option_none: option[string] = {}
test_map: map[hash,string] = {
    0x12345678 = "value1"
    0xdeadbeef = "value2"
}"#,
    ));
}

#[test]
fn test_parse_nested_embeds() {
    insta::assert_snapshot!(tree(
        r#"
data: embed = OuterClass {
    name: string = "outer"
    inner: embed = InnerClass {
        value: u32 = 42
        nested: embed = DeepClass {
            deep_value: f32 = 1.5
        }
    }
}
"#,
    ));
}

// #[test]
// fn test_parse_pointer_null() {
//     let input = r#"
// null_ptr: pointer = null
// "#;
//
//     let file = parse(input).expect("Failed to parse null pointer");
//     assert!(file.entries.contains_key("null_ptr"));
// }

#[test]
fn test_parse_hex_property_names() {
    insta::assert_snapshot!(tree(
        r#"
entries: map[hash,embed] = {
    "Test/Path" = TestClass {
        0xcb13aff1: f32 = -40
        normalName: string = "test"
    }
}
"#,
    ));
}

// #[test]
// fn test_error_span_unknown_type() {
//     let input = "test: badtype = 42";
//     let err = parse(input).unwrap_err();
//
//     // Verify we get an UnknownType error with correct span
//     match err {
//         ParseError::UnknownType {
//             type_name, span, ..
//         } => {
//             assert_eq!(type_name, "badtype");
//             // "badtype" starts at position 6 (after "test: ")
//             assert_eq!(span.offset(), 6);
//             assert_eq!(span.len(), 7); // "badtype" is 7 chars
//         }
//         _ => panic!("Expected UnknownType error, got: {:?}", err),
//     }
// }

// #[test]
// fn test_error_span_multiline() {
//     let input = r#"
// valid: string = "hello"
// broken: unknowntype = 123
// "#;
//     let err = parse(input).unwrap_err();
//
//     match err {
//         ParseError::UnknownType {
//             type_name, span, ..
//         } => {
//             assert_eq!(type_name, "unknowntype");
//             // The span offset should point into the second line
//             assert!(span.offset() > 20); // After first line
//         }
//         _ => panic!("Expected UnknownType error, got: {:?}", err),
//     }
// }
//
// #[test]
// fn test_error_is_miette_diagnostic() {
//     use miette::Diagnostic;
//
//     let input = "test: badtype = 42";
//     let err = parse(input).unwrap_err();
//
//     // ParseError implements Diagnostic
//     let _code = err.code();
//     let _labels = err.labels();
//     let _source = err.source_code();
// }
#[test]
fn test_parse_sample() {
    insta::assert_snapshot!(tree(SAMPLE_RITOBIN));
}
