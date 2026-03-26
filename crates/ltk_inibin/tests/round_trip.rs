use std::io::Cursor;

use glam::{Vec2, Vec3, Vec4};
use ltk_inibin::{Inibin, Value, ValueFlags};

// U8 packed floats: value 100 decodes to 10.0 (byte * 0.1)

/// Round-trip test: construct -> write -> read -> assert equal.
/// Tests all 14 value set types.
#[test]
fn round_trip_all_set_types() {
    let mut file = Inibin::new();

    // Int32List
    file.insert(0x0001, Value::I32(42));
    file.insert(0x0002, Value::I32(-999));

    // F32List
    file.insert(0x0101, Value::F32(3.125));
    file.insert(0x0102, Value::F32(-0.5));

    // U8List (packed float, raw bytes)
    file.insert(0x0201, Value::U8(100)); // 100 * 0.1 = 10.0
    file.insert(0x0202, Value::U8(0)); // 0 * 0.1 = 0.0
    file.insert(0x0203, Value::U8(255)); // 255 * 0.1 = 25.5

    // Int16List
    file.insert(0x0301, Value::I16(-12345));

    // Int8List
    file.insert(0x0401, Value::I8(0));
    file.insert(0x0402, Value::I8(255));

    // BitList
    file.insert(0x0501, Value::Bool(true));
    file.insert(0x0502, Value::Bool(false));
    file.insert(0x0503, Value::Bool(true));

    // Vec3U8List (packed float triple, raw bytes)
    file.insert(0x0601, Value::Vec3U8([10, 20, 30]));

    // F32ListVec3
    file.insert(0x0701, Value::Vec3F32(Vec3::new(1.5, 2.5, 3.5)));

    // Vec2U8List (packed float pair, raw bytes)
    file.insert(0x0801, Value::Vec2U8([50, 100]));

    // F32ListVec2
    file.insert(0x0901, Value::Vec2F32(Vec2::new(7.7, 8.8)));

    // Vec4U8List (packed float quad, raw bytes)
    file.insert(0x0A01, Value::Vec4U8([10, 20, 30, 40]));

    // F32ListVec4
    file.insert(0x0B01, Value::Vec4F32(Vec4::new(1.1, 2.2, 3.3, 4.4)));

    // StringList
    file.insert(0x0C01, Value::String("hello".to_string()));
    file.insert(0x0C02, Value::String("world".to_string()));

    // Int64List
    file.insert(0x0D01, Value::I64(9999999999));
    file.insert(0x0D02, Value::I64(-42));

    // Write
    let mut buf = Vec::new();
    file.to_writer(&mut buf).unwrap();

    // Read back
    let mut cursor = Cursor::new(&buf);
    let file2 = Inibin::from_reader(&mut cursor).unwrap();

    // Verify all values
    assert_eq!(file2.get(0x0001), Some(&Value::I32(42)));
    assert_eq!(file2.get(0x0002), Some(&Value::I32(-999)));

    assert_eq!(file2.get(0x0101), Some(&Value::F32(3.125)));
    assert_eq!(file2.get(0x0102), Some(&Value::F32(-0.5)));

    assert_eq!(file2.get(0x0201), Some(&Value::U8(100)));
    assert_eq!(file2.get(0x0202), Some(&Value::U8(0)));
    assert_eq!(file2.get(0x0203), Some(&Value::U8(255)));

    assert_eq!(file2.get(0x0301), Some(&Value::I16(-12345)));

    assert_eq!(file2.get(0x0401), Some(&Value::I8(0)));
    assert_eq!(file2.get(0x0402), Some(&Value::I8(255)));

    assert_eq!(file2.get(0x0501), Some(&Value::Bool(true)));
    assert_eq!(file2.get(0x0502), Some(&Value::Bool(false)));
    assert_eq!(file2.get(0x0503), Some(&Value::Bool(true)));

    assert_eq!(file2.get(0x0601), Some(&Value::Vec3U8([10, 20, 30])));

    assert_eq!(
        file2.get(0x0701),
        Some(&Value::Vec3F32(Vec3::new(1.5, 2.5, 3.5)))
    );

    assert_eq!(file2.get(0x0801), Some(&Value::Vec2U8([50, 100])));

    assert_eq!(
        file2.get(0x0901),
        Some(&Value::Vec2F32(Vec2::new(7.7, 8.8)))
    );

    assert_eq!(file2.get(0x0A01), Some(&Value::Vec4U8([10, 20, 30, 40])));

    assert_eq!(
        file2.get(0x0B01),
        Some(&Value::Vec4F32(Vec4::new(1.1, 2.2, 3.3, 4.4)))
    );

    assert_eq!(file2.get(0x0C01), Some(&Value::String("hello".to_string())));
    assert_eq!(file2.get(0x0C02), Some(&Value::String("world".to_string())));

    assert_eq!(file2.get(0x0D01), Some(&Value::I64(9999999999)));
    assert_eq!(file2.get(0x0D02), Some(&Value::I64(-42)));
}

#[test]
fn round_trip_empty_file() {
    let file = Inibin::new();

    let mut buf = Vec::new();
    file.to_writer(&mut buf).unwrap();

    let mut cursor = Cursor::new(&buf);
    let file2 = Inibin::from_reader(&mut cursor).unwrap();

    assert_eq!(file2.iter().count(), 0);
}

#[test]
fn round_trip_bit_list_partial_byte() {
    // 3 bools = 1 byte with partial packing
    let mut file = Inibin::new();
    file.insert(0x0001, Value::Bool(true));
    file.insert(0x0002, Value::Bool(false));
    file.insert(0x0003, Value::Bool(true));

    let mut buf = Vec::new();
    file.to_writer(&mut buf).unwrap();

    let mut cursor = Cursor::new(&buf);
    let file2 = Inibin::from_reader(&mut cursor).unwrap();

    assert_eq!(file2.get(0x0001), Some(&Value::Bool(true)));
    assert_eq!(file2.get(0x0002), Some(&Value::Bool(false)));
    assert_eq!(file2.get(0x0003), Some(&Value::Bool(true)));
}

#[test]
fn as_f32_accessor() {
    // Packed U8 variants
    let val = Value::U8(100);
    approx::assert_relative_eq!(val.as_f32().unwrap(), 10.0);

    let val = Value::U8(255);
    approx::assert_relative_eq!(val.as_f32().unwrap(), 25.5);

    let val = Value::U8(0);
    approx::assert_relative_eq!(val.as_f32().unwrap(), 0.0);

    // Non-packed F32 variant
    let val = Value::F32(3.125);
    approx::assert_relative_eq!(val.as_f32().unwrap(), 3.125);

    // Non-float variant returns None
    assert_eq!(Value::I32(42).as_f32(), None);
}

#[test]
fn as_vec2_accessor() {
    // Packed U8 variant
    let val = Value::Vec2U8([50, 100]);
    let v = val.as_vec2().unwrap();
    approx::assert_relative_eq!(v.x, 5.0);
    approx::assert_relative_eq!(v.y, 10.0);

    // Non-packed F32 variant
    let val = Value::Vec2F32(Vec2::new(1.5, 2.5));
    assert_eq!(val.as_vec2(), Some(Vec2::new(1.5, 2.5)));

    // Non-vec2 variant returns None
    assert_eq!(Value::I32(42).as_vec2(), None);
}

#[test]
fn as_vec3_accessor() {
    // Packed U8 variant
    let val = Value::Vec3U8([10, 20, 30]);
    let v = val.as_vec3().unwrap();
    approx::assert_relative_eq!(v.x, 1.0);
    approx::assert_relative_eq!(v.y, 2.0);
    approx::assert_relative_eq!(v.z, 3.0);

    // Non-packed F32 variant
    let val = Value::Vec3F32(Vec3::new(1.5, 2.5, 3.5));
    assert_eq!(val.as_vec3(), Some(Vec3::new(1.5, 2.5, 3.5)));

    // Non-vec3 variant returns None
    assert_eq!(Value::I32(42).as_vec3(), None);
}

#[test]
fn as_vec4_accessor() {
    // Packed U8 variant
    let val = Value::Vec4U8([10, 20, 30, 40]);
    let v = val.as_vec4().unwrap();
    approx::assert_relative_eq!(v.x, 1.0);
    approx::assert_relative_eq!(v.y, 2.0);
    approx::assert_relative_eq!(v.z, 3.0);
    approx::assert_relative_eq!(v.w, 4.0);

    // Non-packed F32 variant
    let val = Value::Vec4F32(Vec4::new(1.1, 2.2, 3.3, 4.4));
    assert_eq!(val.as_vec4(), Some(Vec4::new(1.1, 2.2, 3.3, 4.4)));

    // Non-vec4 variant returns None
    assert_eq!(Value::I32(42).as_vec4(), None);
}

#[test]
fn test_set_access() {
    let mut file = Inibin::new();
    file.insert(0x0001, Value::I32(1));
    file.insert(0x0002, Value::I32(2));
    file.insert(0x0003, Value::F32(3.0));

    let int_set = file.section(ValueFlags::INT32_LIST).unwrap();
    assert_eq!(int_set.len(), 2);
    assert_eq!(int_set.kind(), ValueFlags::INT32_LIST);

    let float_set = file.section(ValueFlags::F32_LIST).unwrap();
    assert_eq!(float_set.len(), 1);
}

#[test]
fn round_trip_int64() {
    let mut file = Inibin::new();
    file.insert(0x0001, Value::I64(i64::MAX));
    file.insert(0x0002, Value::I64(i64::MIN));
    file.insert(0x0003, Value::I64(0));

    let mut buf = Vec::new();
    file.to_writer(&mut buf).unwrap();

    let mut cursor = Cursor::new(&buf);
    let file2 = Inibin::from_reader(&mut cursor).unwrap();

    assert_eq!(file2.get(0x0001), Some(&Value::I64(i64::MAX)));
    assert_eq!(file2.get(0x0002), Some(&Value::I64(i64::MIN)));
    assert_eq!(file2.get(0x0003), Some(&Value::I64(0)));
}

#[test]
fn test_int64_cross_bucket_migration() {
    let mut file = Inibin::new();

    // Insert as Int32 first
    file.insert(0xABCD, Value::I32(42));
    assert_eq!(file.get(0xABCD), Some(&Value::I32(42)));

    // Re-insert same key as Int64
    file.insert(0xABCD, Value::I64(9999999999));
    assert_eq!(file.get(0xABCD), Some(&Value::I64(9999999999)));

    // Verify it's not in Int32 bucket anymore
    assert!(file
        .section(ValueFlags::INT32_LIST)
        .map(|s| s.get(0xABCD).is_none())
        .unwrap_or(true));

    // Remove and verify
    let removed = file.remove(0xABCD);
    assert_eq!(removed, Some(Value::I64(9999999999)));
    assert!(!file.contains_key(0xABCD));
}
