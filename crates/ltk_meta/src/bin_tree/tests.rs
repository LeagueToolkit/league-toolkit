//! Tests for BinTree reading and writing

use std::io::Cursor;

use glam::{Mat4, Vec2, Vec3, Vec4};
use indexmap::IndexMap;
use ltk_primitives::Color;

use crate::property::value::*;
use crate::property::{BinProperty, BinPropertyKind, PropertyValueEnum};
use crate::{BinTree, BinTreeObject};

/// Helper to roundtrip a property value through write/read
fn roundtrip_property(prop: &BinProperty) -> BinProperty {
    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    prop.to_writer(&mut cursor).expect("write failed");

    cursor.set_position(0);
    BinProperty::from_reader(&mut cursor, false).expect("read failed")
}

/// Helper to roundtrip a BinTree through write/read
fn roundtrip_tree(tree: &BinTree) -> BinTree {
    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    tree.to_writer(&mut cursor).expect("write failed");

    cursor.set_position(0);
    BinTree::from_reader(&mut cursor).expect("read failed")
}

/// Helper to create a BinProperty with a given name hash and value
fn make_prop(name_hash: u32, value: PropertyValueEnum) -> BinProperty {
    BinProperty { name_hash, value }
}

// =============================================================================
// Primitive Property Tests
// =============================================================================

#[test]
fn test_none_property_roundtrip() {
    let prop = make_prop(0x1234, PropertyValueEnum::None(NoneValue));
    let result = roundtrip_property(&prop);
    assert_eq!(prop, result);
}

#[test]
fn test_bool_property_roundtrip() {
    for value in [true, false] {
        let prop = make_prop(0x1234, PropertyValueEnum::Bool(BoolValue(value)));
        let result = roundtrip_property(&prop);
        assert_eq!(prop, result);
    }
}

#[test]
fn test_bitbool_property_roundtrip() {
    for value in [true, false] {
        let prop = make_prop(0x1234, PropertyValueEnum::BitBool(BitBoolValue(value)));
        let result = roundtrip_property(&prop);
        assert_eq!(prop, result);
    }
}

#[test]
fn test_i8_property_roundtrip() {
    for value in [i8::MIN, -1, 0, 1, i8::MAX] {
        let prop = make_prop(0x1234, PropertyValueEnum::I8(I8Value(value)));
        let result = roundtrip_property(&prop);
        assert_eq!(prop, result);
    }
}

#[test]
fn test_u8_property_roundtrip() {
    for value in [u8::MIN, 1, 128, u8::MAX] {
        let prop = make_prop(0x1234, PropertyValueEnum::U8(U8Value(value)));
        let result = roundtrip_property(&prop);
        assert_eq!(prop, result);
    }
}

#[test]
fn test_i16_property_roundtrip() {
    for value in [i16::MIN, -1, 0, 1, i16::MAX] {
        let prop = make_prop(0x1234, PropertyValueEnum::I16(I16Value(value)));
        let result = roundtrip_property(&prop);
        assert_eq!(prop, result);
    }
}

#[test]
fn test_u16_property_roundtrip() {
    for value in [u16::MIN, 1, 32768, u16::MAX] {
        let prop = make_prop(0x1234, PropertyValueEnum::U16(U16Value(value)));
        let result = roundtrip_property(&prop);
        assert_eq!(prop, result);
    }
}

#[test]
fn test_i32_property_roundtrip() {
    for value in [i32::MIN, -1, 0, 1, i32::MAX] {
        let prop = make_prop(0x1234, PropertyValueEnum::I32(I32Value(value)));
        let result = roundtrip_property(&prop);
        assert_eq!(prop, result);
    }
}

#[test]
fn test_u32_property_roundtrip() {
    for value in [u32::MIN, 1, 0x8000_0000, u32::MAX] {
        let prop = make_prop(0x1234, PropertyValueEnum::U32(U32Value(value)));
        let result = roundtrip_property(&prop);
        assert_eq!(prop, result);
    }
}

#[test]
fn test_i64_property_roundtrip() {
    for value in [i64::MIN, -1, 0, 1, i64::MAX] {
        let prop = make_prop(0x1234, PropertyValueEnum::I64(I64Value(value)));
        let result = roundtrip_property(&prop);
        assert_eq!(prop, result);
    }
}

#[test]
fn test_u64_property_roundtrip() {
    for value in [u64::MIN, 1, 0x8000_0000_0000_0000, u64::MAX] {
        let prop = make_prop(0x1234, PropertyValueEnum::U64(U64Value(value)));
        let result = roundtrip_property(&prop);
        assert_eq!(prop, result);
    }
}

#[test]
fn test_f32_property_roundtrip() {
    for value in [0.0, 1.0, -1.0, f32::MIN, f32::MAX, std::f32::consts::PI] {
        let prop = make_prop(0x1234, PropertyValueEnum::F32(F32Value(value)));
        let result = roundtrip_property(&prop);
        assert_eq!(prop, result);
    }
}

#[test]
fn test_vector2_property_roundtrip() {
    let values = [
        Vec2::ZERO,
        Vec2::ONE,
        Vec2::new(-1.0, 2.5),
        Vec2::new(f32::MIN, f32::MAX),
    ];
    for value in values {
        let prop = make_prop(0x1234, PropertyValueEnum::Vector2(Vector2Value(value)));
        let result = roundtrip_property(&prop);
        assert_eq!(prop, result);
    }
}

#[test]
fn test_vector3_property_roundtrip() {
    let values = [
        Vec3::ZERO,
        Vec3::ONE,
        Vec3::new(-1.0, 2.5, std::f32::consts::PI),
        Vec3::new(f32::MIN, 0.0, f32::MAX),
    ];
    for value in values {
        let prop = make_prop(0x1234, PropertyValueEnum::Vector3(Vector3Value(value)));
        let result = roundtrip_property(&prop);
        assert_eq!(prop, result);
    }
}

#[test]
fn test_vector4_property_roundtrip() {
    let values = [
        Vec4::ZERO,
        Vec4::ONE,
        Vec4::new(-1.0, 2.5, std::f32::consts::PI, 0.0),
        Vec4::new(f32::MIN, 0.0, 0.0, f32::MAX),
    ];
    for value in values {
        let prop = make_prop(0x1234, PropertyValueEnum::Vector4(Vector4Value(value)));
        let result = roundtrip_property(&prop);
        assert_eq!(prop, result);
    }
}

#[test]
fn test_matrix44_property_roundtrip() {
    let values = [
        Mat4::IDENTITY,
        Mat4::ZERO,
        Mat4::from_cols(
            Vec4::new(1.0, 2.0, 3.0, 4.0),
            Vec4::new(5.0, 6.0, 7.0, 8.0),
            Vec4::new(9.0, 10.0, 11.0, 12.0),
            Vec4::new(13.0, 14.0, 15.0, 16.0),
        ),
    ];
    for value in values {
        let prop = make_prop(0x1234, PropertyValueEnum::Matrix44(Matrix44Value(value)));
        let result = roundtrip_property(&prop);
        assert_eq!(prop, result);
    }
}

#[test]
fn test_color_property_roundtrip() {
    let values = [
        Color::new(0, 0, 0, 0),
        Color::new(255, 255, 255, 255),
        Color::new(128, 64, 32, 16),
        Color::new(0, 128, 255, 100),
    ];
    for value in values {
        let prop = make_prop(0x1234, PropertyValueEnum::Color(ColorValue(value)));
        let result = roundtrip_property(&prop);
        assert_eq!(prop, result);
    }
}

#[test]
fn test_string_property_roundtrip() {
    let values = [
        String::new(),
        "hello".to_string(),
        "Hello, World! 🌍".to_string(),
        "a".repeat(1000),
    ];
    for value in values {
        let prop = make_prop(0x1234, PropertyValueEnum::String(StringValue(value)));
        let result = roundtrip_property(&prop);
        assert_eq!(prop, result);
    }
}

#[test]
fn test_hash_property_roundtrip() {
    for value in [0u32, 1, 0xDEADBEEF, u32::MAX] {
        let prop = make_prop(0x1234, PropertyValueEnum::Hash(HashValue(value)));
        let result = roundtrip_property(&prop);
        assert_eq!(prop, result);
    }
}

#[test]
fn test_wad_chunk_link_property_roundtrip() {
    for value in [0u64, 1, 0xDEAD_BEEF_CAFE_BABE, u64::MAX] {
        let prop = make_prop(
            0x1234,
            PropertyValueEnum::WadChunkLink(WadChunkLinkValue(value)),
        );
        let result = roundtrip_property(&prop);
        assert_eq!(prop, result);
    }
}

#[test]
fn test_object_link_property_roundtrip() {
    for value in [0u32, 1, 0xDEADBEEF, u32::MAX] {
        let prop = make_prop(
            0x1234,
            PropertyValueEnum::ObjectLink(ObjectLinkValue(value)),
        );
        let result = roundtrip_property(&prop);
        assert_eq!(prop, result);
    }
}

// =============================================================================
// Container Property Tests
// =============================================================================

#[test]
fn test_container_empty_roundtrip() {
    let prop = make_prop(
        0x1234,
        PropertyValueEnum::Container(ContainerValue {
            item_kind: BinPropertyKind::I32,
            items: vec![],
        }),
    );
    let result = roundtrip_property(&prop);
    assert_eq!(prop, result);
}

#[test]
fn test_container_with_primitives_roundtrip() {
    let prop = make_prop(
        0x1234,
        PropertyValueEnum::Container(ContainerValue {
            item_kind: BinPropertyKind::I32,
            items: vec![
                PropertyValueEnum::I32(I32Value(1)),
                PropertyValueEnum::I32(I32Value(2)),
                PropertyValueEnum::I32(I32Value(3)),
            ],
        }),
    );
    let result = roundtrip_property(&prop);
    assert_eq!(prop, result);
}

#[test]
fn test_container_with_strings_roundtrip() {
    let prop = make_prop(
        0x1234,
        PropertyValueEnum::Container(ContainerValue {
            item_kind: BinPropertyKind::String,
            items: vec![
                PropertyValueEnum::String(StringValue("hello".into())),
                PropertyValueEnum::String(StringValue("world".into())),
            ],
        }),
    );
    let result = roundtrip_property(&prop);
    assert_eq!(prop, result);
}

#[test]
fn test_container_with_structs_roundtrip() {
    let mut properties = IndexMap::new();
    properties.insert(
        0xAAAA,
        BinProperty {
            name_hash: 0xAAAA,
            value: PropertyValueEnum::I32(I32Value(42)),
        },
    );

    let prop = make_prop(
        0x1234,
        PropertyValueEnum::Container(ContainerValue {
            item_kind: BinPropertyKind::Struct,
            items: vec![PropertyValueEnum::Struct(StructValue {
                class_hash: 0xBBBB,
                properties,
            })],
        }),
    );
    let result = roundtrip_property(&prop);
    assert_eq!(prop, result);
}

#[test]
fn test_unordered_container_roundtrip() {
    let prop = make_prop(
        0x1234,
        PropertyValueEnum::UnorderedContainer(UnorderedContainerValue(ContainerValue {
            item_kind: BinPropertyKind::U32,
            items: vec![
                PropertyValueEnum::U32(U32Value(100)),
                PropertyValueEnum::U32(U32Value(200)),
            ],
        })),
    );
    let result = roundtrip_property(&prop);
    assert_eq!(prop, result);
}

// =============================================================================
// Optional Property Tests
// =============================================================================

#[test]
fn test_optional_none_roundtrip() {
    let prop = make_prop(
        0x1234,
        PropertyValueEnum::Optional(OptionalValue {
            kind: BinPropertyKind::I32,
            value: None,
        }),
    );
    let result = roundtrip_property(&prop);
    assert_eq!(prop, result);
}

#[test]
fn test_optional_some_primitive_roundtrip() {
    let prop = make_prop(
        0x1234,
        PropertyValueEnum::Optional(OptionalValue {
            kind: BinPropertyKind::I32,
            value: Some(Box::new(PropertyValueEnum::I32(I32Value(42)))),
        }),
    );
    let result = roundtrip_property(&prop);
    assert_eq!(prop, result);
}

#[test]
fn test_optional_some_string_roundtrip() {
    let prop = make_prop(
        0x1234,
        PropertyValueEnum::Optional(OptionalValue {
            kind: BinPropertyKind::String,
            value: Some(Box::new(PropertyValueEnum::String(StringValue(
                "optional value".into(),
            )))),
        }),
    );
    let result = roundtrip_property(&prop);
    assert_eq!(prop, result);
}

#[test]
fn test_optional_some_struct_roundtrip() {
    let mut properties = IndexMap::new();
    properties.insert(
        0xAAAA,
        BinProperty {
            name_hash: 0xAAAA,
            value: PropertyValueEnum::Bool(BoolValue(true)),
        },
    );

    let prop = make_prop(
        0x1234,
        PropertyValueEnum::Optional(OptionalValue {
            kind: BinPropertyKind::Struct,
            value: Some(Box::new(PropertyValueEnum::Struct(StructValue {
                class_hash: 0xCCCC,
                properties,
            }))),
        }),
    );
    let result = roundtrip_property(&prop);
    assert_eq!(prop, result);
}

// =============================================================================
// Map Property Tests
// =============================================================================

#[test]
fn test_map_empty_roundtrip() {
    let prop = make_prop(
        0x1234,
        PropertyValueEnum::Map(MapValue {
            key_kind: BinPropertyKind::U32,
            value_kind: BinPropertyKind::String,
            entries: IndexMap::new(),
        }),
    );
    let result = roundtrip_property(&prop);
    assert_eq!(prop, result);
}

#[test]
fn test_map_u32_to_string_roundtrip() {
    let mut entries = IndexMap::new();
    entries.insert(
        PropertyValueUnsafeEq(PropertyValueEnum::U32(U32Value(1))),
        PropertyValueEnum::String(StringValue("one".into())),
    );
    entries.insert(
        PropertyValueUnsafeEq(PropertyValueEnum::U32(U32Value(2))),
        PropertyValueEnum::String(StringValue("two".into())),
    );

    let prop = make_prop(
        0x1234,
        PropertyValueEnum::Map(MapValue {
            key_kind: BinPropertyKind::U32,
            value_kind: BinPropertyKind::String,
            entries,
        }),
    );
    let result = roundtrip_property(&prop);
    assert_eq!(prop, result);
}

#[test]
fn test_map_hash_to_struct_roundtrip() {
    let mut struct_props = IndexMap::new();
    struct_props.insert(
        0x1111,
        BinProperty {
            name_hash: 0x1111,
            value: PropertyValueEnum::F32(F32Value(std::f32::consts::PI)),
        },
    );

    let mut entries = IndexMap::new();
    entries.insert(
        PropertyValueUnsafeEq(PropertyValueEnum::Hash(HashValue(0xDEAD))),
        PropertyValueEnum::Struct(StructValue {
            class_hash: 0xBEEF,
            properties: struct_props,
        }),
    );

    let prop = make_prop(
        0x1234,
        PropertyValueEnum::Map(MapValue {
            key_kind: BinPropertyKind::Hash,
            value_kind: BinPropertyKind::Struct,
            entries,
        }),
    );
    let result = roundtrip_property(&prop);
    assert_eq!(prop, result);
}

// =============================================================================
// Struct Property Tests
// =============================================================================

#[test]
fn test_struct_empty_roundtrip() {
    let prop = make_prop(
        0x1234,
        PropertyValueEnum::Struct(StructValue {
            class_hash: 0,
            properties: IndexMap::new(),
        }),
    );
    let result = roundtrip_property(&prop);
    assert_eq!(prop, result);
}

#[test]
fn test_struct_with_properties_roundtrip() {
    let mut properties = IndexMap::new();
    properties.insert(
        0x1111,
        BinProperty {
            name_hash: 0x1111,
            value: PropertyValueEnum::I32(I32Value(42)),
        },
    );
    properties.insert(
        0x2222,
        BinProperty {
            name_hash: 0x2222,
            value: PropertyValueEnum::String(StringValue("test".into())),
        },
    );
    properties.insert(
        0x3333,
        BinProperty {
            name_hash: 0x3333,
            value: PropertyValueEnum::Bool(BoolValue(true)),
        },
    );

    let prop = make_prop(
        0x1234,
        PropertyValueEnum::Struct(StructValue {
            class_hash: 0xABCD,
            properties,
        }),
    );
    let result = roundtrip_property(&prop);
    assert_eq!(prop, result);
}

#[test]
fn test_struct_nested_roundtrip() {
    let mut inner_props = IndexMap::new();
    inner_props.insert(
        0xAAAA,
        BinProperty {
            name_hash: 0xAAAA,
            value: PropertyValueEnum::F32(F32Value(1.5)),
        },
    );

    let mut outer_props = IndexMap::new();
    outer_props.insert(
        0xBBBB,
        BinProperty {
            name_hash: 0xBBBB,
            value: PropertyValueEnum::Struct(StructValue {
                class_hash: 0x1111,
                properties: inner_props,
            }),
        },
    );

    let prop = make_prop(
        0x1234,
        PropertyValueEnum::Struct(StructValue {
            class_hash: 0x2222,
            properties: outer_props,
        }),
    );
    let result = roundtrip_property(&prop);
    assert_eq!(prop, result);
}

// =============================================================================
// Embedded Property Tests
// =============================================================================

#[test]
fn test_embedded_roundtrip() {
    let mut properties = IndexMap::new();
    properties.insert(
        0x1111,
        BinProperty {
            name_hash: 0x1111,
            value: PropertyValueEnum::Vector3(Vector3Value(Vec3::new(1.0, 2.0, 3.0))),
        },
    );

    let prop = make_prop(
        0x1234,
        PropertyValueEnum::Embedded(EmbeddedValue(StructValue {
            class_hash: 0xEEEE,
            properties,
        })),
    );
    let result = roundtrip_property(&prop);
    assert_eq!(prop, result);
}

// =============================================================================
// BinTreeObject Tests
// =============================================================================

#[test]
fn test_bin_tree_object_empty_roundtrip() {
    let obj = BinTreeObject {
        path_hash: 0x1234,
        class_hash: 0x5678,
        properties: IndexMap::new(),
    };

    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    obj.to_writer(&mut cursor).expect("write failed");

    cursor.set_position(0);
    let result =
        BinTreeObject::from_reader(&mut cursor, obj.class_hash, false).expect("read failed");
    assert_eq!(obj, result);
}

#[test]
fn test_bin_tree_object_with_properties_roundtrip() {
    let mut properties = IndexMap::new();
    properties.insert(
        0xAAAA,
        BinProperty {
            name_hash: 0xAAAA,
            value: PropertyValueEnum::I32(I32Value(100)),
        },
    );
    properties.insert(
        0xBBBB,
        BinProperty {
            name_hash: 0xBBBB,
            value: PropertyValueEnum::String(StringValue("object property".into())),
        },
    );

    let obj = BinTreeObject {
        path_hash: 0x1234,
        class_hash: 0x5678,
        properties,
    };

    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    obj.to_writer(&mut cursor).expect("write failed");

    cursor.set_position(0);
    let result =
        BinTreeObject::from_reader(&mut cursor, obj.class_hash, false).expect("read failed");
    assert_eq!(obj, result);
}

// =============================================================================
// BinTree Tests
// =============================================================================

#[test]
fn test_bin_tree_empty_roundtrip() {
    let tree = BinTree::new([], std::iter::empty::<String>());
    let result = roundtrip_tree(&tree);
    assert_eq!(tree, result);
}

#[test]
fn test_bin_tree_with_dependencies_roundtrip() {
    let tree = BinTree::new(
        [],
        ["dependency1.bin".to_string(), "dependency2.bin".to_string()],
    );
    let result = roundtrip_tree(&tree);
    assert_eq!(tree, result);
}

#[test]
fn test_bin_tree_with_objects_roundtrip() {
    let mut properties = IndexMap::new();
    properties.insert(
        0xAAAA,
        BinProperty {
            name_hash: 0xAAAA,
            value: PropertyValueEnum::I32(I32Value(42)),
        },
    );

    let obj = BinTreeObject {
        path_hash: 0x1234,
        class_hash: 0x5678,
        properties,
    };

    let tree = BinTree::new([obj], std::iter::empty::<String>());
    let result = roundtrip_tree(&tree);
    assert_eq!(tree, result);
}

#[test]
fn test_bin_tree_complex_roundtrip() {
    // Create a complex tree with multiple objects and various property types
    let mut obj1_props = IndexMap::new();
    obj1_props.insert(
        0x1111,
        BinProperty {
            name_hash: 0x1111,
            value: PropertyValueEnum::Bool(BoolValue(true)),
        },
    );
    obj1_props.insert(
        0x2222,
        BinProperty {
            name_hash: 0x2222,
            value: PropertyValueEnum::String(StringValue("test string".into())),
        },
    );
    obj1_props.insert(
        0x3333,
        BinProperty {
            name_hash: 0x3333,
            value: PropertyValueEnum::Container(ContainerValue {
                item_kind: BinPropertyKind::I32,
                items: vec![
                    PropertyValueEnum::I32(I32Value(1)),
                    PropertyValueEnum::I32(I32Value(2)),
                    PropertyValueEnum::I32(I32Value(3)),
                ],
            }),
        },
    );

    let obj1 = BinTreeObject {
        path_hash: 0xAAAA,
        class_hash: 0xBBBB,
        properties: obj1_props,
    };

    let mut obj2_props = IndexMap::new();
    obj2_props.insert(
        0x4444,
        BinProperty {
            name_hash: 0x4444,
            value: PropertyValueEnum::Vector3(Vector3Value(Vec3::new(1.0, 2.0, 3.0))),
        },
    );
    obj2_props.insert(
        0x5555,
        BinProperty {
            name_hash: 0x5555,
            value: PropertyValueEnum::Optional(OptionalValue {
                kind: BinPropertyKind::F32,
                value: Some(Box::new(PropertyValueEnum::F32(F32Value(
                    std::f32::consts::PI,
                )))),
            }),
        },
    );

    let obj2 = BinTreeObject {
        path_hash: 0xCCCC,
        class_hash: 0xDDDD,
        properties: obj2_props,
    };

    let tree = BinTree::new(
        [obj1, obj2],
        ["dep1.bin".to_string(), "dep2.bin".to_string()],
    );
    let result = roundtrip_tree(&tree);
    assert_eq!(tree, result);
}

// =============================================================================
// Property Kind Tests
// =============================================================================

#[test]
fn test_property_kind_roundtrip() {
    use crate::traits::{ReaderExt, WriterExt};

    let kinds = [
        BinPropertyKind::None,
        BinPropertyKind::Bool,
        BinPropertyKind::I8,
        BinPropertyKind::U8,
        BinPropertyKind::I16,
        BinPropertyKind::U16,
        BinPropertyKind::I32,
        BinPropertyKind::U32,
        BinPropertyKind::I64,
        BinPropertyKind::U64,
        BinPropertyKind::F32,
        BinPropertyKind::Vector2,
        BinPropertyKind::Vector3,
        BinPropertyKind::Vector4,
        BinPropertyKind::Matrix44,
        BinPropertyKind::Color,
        BinPropertyKind::String,
        BinPropertyKind::Hash,
        BinPropertyKind::WadChunkLink,
        BinPropertyKind::Container,
        BinPropertyKind::UnorderedContainer,
        BinPropertyKind::Struct,
        BinPropertyKind::Embedded,
        BinPropertyKind::ObjectLink,
        BinPropertyKind::Optional,
        BinPropertyKind::Map,
        BinPropertyKind::BitBool,
    ];

    for kind in kinds {
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        cursor.write_property_kind(kind).expect("write failed");

        cursor.set_position(0);
        let result = cursor.read_property_kind(false).expect("read failed");
        assert_eq!(kind, result, "Kind mismatch for {:?}", kind);
    }
}

// =============================================================================
// Edge Case Tests
// =============================================================================

#[test]
fn test_deeply_nested_struct() {
    // Create a deeply nested struct to test recursion handling
    let mut deepest_props = IndexMap::new();
    deepest_props.insert(
        0x1111,
        BinProperty {
            name_hash: 0x1111,
            value: PropertyValueEnum::I32(I32Value(42)),
        },
    );

    let mut level2_props = IndexMap::new();
    level2_props.insert(
        0x2222,
        BinProperty {
            name_hash: 0x2222,
            value: PropertyValueEnum::Struct(StructValue {
                class_hash: 0xAAAA,
                properties: deepest_props,
            }),
        },
    );

    let mut level1_props = IndexMap::new();
    level1_props.insert(
        0x3333,
        BinProperty {
            name_hash: 0x3333,
            value: PropertyValueEnum::Struct(StructValue {
                class_hash: 0xBBBB,
                properties: level2_props,
            }),
        },
    );

    let prop = make_prop(
        0x4444,
        PropertyValueEnum::Struct(StructValue {
            class_hash: 0xCCCC,
            properties: level1_props,
        }),
    );
    let result = roundtrip_property(&prop);
    assert_eq!(prop, result);
}

#[test]
fn test_container_with_embedded_roundtrip() {
    let mut embedded_props = IndexMap::new();
    embedded_props.insert(
        0x1111,
        BinProperty {
            name_hash: 0x1111,
            value: PropertyValueEnum::U32(U32Value(999)),
        },
    );

    let prop = make_prop(
        0x1234,
        PropertyValueEnum::Container(ContainerValue {
            item_kind: BinPropertyKind::Embedded,
            items: vec![
                PropertyValueEnum::Embedded(EmbeddedValue(StructValue {
                    class_hash: 0xAAAA,
                    properties: embedded_props.clone(),
                })),
                PropertyValueEnum::Embedded(EmbeddedValue(StructValue {
                    class_hash: 0xBBBB,
                    properties: embedded_props,
                })),
            ],
        }),
    );
    let result = roundtrip_property(&prop);
    assert_eq!(prop, result);
}

#[test]
fn test_all_primitive_kinds_in_container() {
    // Test that all primitive kinds can be stored in containers
    let test_cases: Vec<(BinPropertyKind, PropertyValueEnum)> = vec![
        (
            BinPropertyKind::Bool,
            PropertyValueEnum::Bool(BoolValue(true)),
        ),
        (BinPropertyKind::I8, PropertyValueEnum::I8(I8Value(-1))),
        (BinPropertyKind::U8, PropertyValueEnum::U8(U8Value(1))),
        (BinPropertyKind::I16, PropertyValueEnum::I16(I16Value(-100))),
        (BinPropertyKind::U16, PropertyValueEnum::U16(U16Value(100))),
        (
            BinPropertyKind::I32,
            PropertyValueEnum::I32(I32Value(-1000)),
        ),
        (BinPropertyKind::U32, PropertyValueEnum::U32(U32Value(1000))),
        (
            BinPropertyKind::I64,
            PropertyValueEnum::I64(I64Value(-10000)),
        ),
        (
            BinPropertyKind::U64,
            PropertyValueEnum::U64(U64Value(10000)),
        ),
        (BinPropertyKind::F32, PropertyValueEnum::F32(F32Value(1.5))),
        (
            BinPropertyKind::Vector2,
            PropertyValueEnum::Vector2(Vector2Value(Vec2::ONE)),
        ),
        (
            BinPropertyKind::Vector3,
            PropertyValueEnum::Vector3(Vector3Value(Vec3::ONE)),
        ),
        (
            BinPropertyKind::Vector4,
            PropertyValueEnum::Vector4(Vector4Value(Vec4::ONE)),
        ),
        (
            BinPropertyKind::Matrix44,
            PropertyValueEnum::Matrix44(Matrix44Value(Mat4::IDENTITY)),
        ),
        (
            BinPropertyKind::Color,
            PropertyValueEnum::Color(ColorValue(Color::new(255, 128, 64, 32))),
        ),
        (
            BinPropertyKind::String,
            PropertyValueEnum::String(StringValue("test".into())),
        ),
        (
            BinPropertyKind::Hash,
            PropertyValueEnum::Hash(HashValue(0xDEADBEEF)),
        ),
        (
            BinPropertyKind::WadChunkLink,
            PropertyValueEnum::WadChunkLink(WadChunkLinkValue(0xCAFEBABE)),
        ),
        (
            BinPropertyKind::ObjectLink,
            PropertyValueEnum::ObjectLink(ObjectLinkValue(0x12345678)),
        ),
    ];

    for (kind, value) in test_cases {
        let prop = make_prop(
            0x1234,
            PropertyValueEnum::Container(ContainerValue {
                item_kind: kind,
                items: vec![value],
            }),
        );
        let result = roundtrip_property(&prop);
        assert_eq!(prop, result, "Failed for kind {:?}", kind);
    }
}
