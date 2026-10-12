use glam::{vec2, vec3, vec4};

use super::*;

/// Returns the bytes of a version 2 file: the header with `string_data_len` and `flags`, followed
/// by `body`.
fn file(string_data_len: u16, flags: u16, body: &[u8]) -> Vec<u8> {
    let mut bytes = vec![VERSION];
    bytes.extend_from_slice(&string_data_len.to_le_bytes());
    bytes.extend_from_slice(&flags.to_le_bytes());
    bytes.extend_from_slice(body);
    bytes
}

fn read(bytes: &[u8]) -> Result<Inibin> {
    Inibin::from_reader(&mut &bytes[..])
}

fn write(inibin: &Inibin) -> Vec<u8> {
    let mut bytes = Vec::new();
    inibin.to_writer(&mut bytes).unwrap();
    bytes
}

/// Returns an `Inibin` with one value of each storage type.
fn every_kind() -> Inibin {
    Inibin::from_iter([
        (1, Value::I32(-100_000)),
        (2, Value::F32(1.25)),
        (3, Value::Fixed(15)),
        (4, Value::I16(-50)),
        (5, Value::U8(200)),
        (6, Value::Bool(true)),
        (7, Value::FixedVec3([8, 8, 8])),
        (8, Value::Vec3(vec3(30.0, 0.0, 0.25))),
        (9, Value::FixedVec2([10, 0])),
        (10, Value::Vec2(vec2(0.5, -45.0))),
        (11, Value::FixedVec4([10, 10, 10, 5])),
        (12, Value::Vec4(vec4(0.75, 3.0, 5.0, 0.0))),
        (13, Value::from("glow")),
        (14, Value::I64(-(1 << 40))),
    ])
}

/// Returns the bytes of a version 2 file with one string value. The key is 7 and the string
/// offset is `offset` in `string_data`.
fn one_string(offset: u16, string_data: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&1u16.to_le_bytes());
    body.extend_from_slice(&7u32.to_le_bytes());
    body.extend_from_slice(&offset.to_le_bytes());
    body.extend_from_slice(string_data);
    file(string_data.len() as u16, ValueKind::String.flag(), &body)
}

#[test]
fn from_reader_reads_empty_file() {
    assert_eq!(read(&[2, 0, 0, 0, 0]).unwrap(), Inibin::new());
}

#[test]
fn from_reader_reads_i64_set_before_string_data() {
    let flags = ValueKind::String.flag() | ValueKind::I64.flag();
    let mut body = Vec::new();
    body.extend_from_slice(&1u16.to_le_bytes());
    body.extend_from_slice(&0xaau32.to_le_bytes());
    body.extend_from_slice(&0u16.to_le_bytes());
    body.extend_from_slice(&1u16.to_le_bytes());
    body.extend_from_slice(&0xbbu32.to_le_bytes());
    body.extend_from_slice(&(-7i64).to_le_bytes());
    body.extend_from_slice(b"glow\0");

    let inibin = read(&file(5, flags, &body)).unwrap();

    assert_eq!(inibin.get(0xaa), Some(&Value::from("glow")));
    assert_eq!(inibin.get(0xbb), Some(&Value::I64(-7)));
}

#[test]
fn from_reader_reads_bool_from_lowest_bit_first() {
    let mut body = Vec::new();
    body.extend_from_slice(&10u16.to_le_bytes());
    for key in 0..10u32 {
        body.extend_from_slice(&key.to_le_bytes());
    }
    body.extend_from_slice(&[0b0000_0101, 0b0000_0010]);

    let inibin = read(&file(0, ValueKind::Bool.flag(), &body)).unwrap();

    let set: Vec<u32> = inibin
        .iter()
        .filter(|(_, value)| **value == Value::Bool(true))
        .map(|(key, _)| key.0)
        .collect();
    assert_eq!(inibin.len(), 10);
    assert_eq!(set, [0, 2, 9]);
}

#[test]
fn from_reader_reads_strings_with_shared_offset() {
    let mut body = Vec::new();
    body.extend_from_slice(&3u16.to_le_bytes());
    for key in [1u32, 2, 3] {
        body.extend_from_slice(&key.to_le_bytes());
    }
    for offset in [5u16, 0, 5] {
        body.extend_from_slice(&offset.to_le_bytes());
    }
    body.extend_from_slice(b"core\0glow\0");

    let inibin = read(&file(10, ValueKind::String.flag(), &body)).unwrap();

    assert_eq!(inibin.get(1), Some(&Value::from("glow")));
    assert_eq!(inibin.get(2), Some(&Value::from("core")));
    assert_eq!(inibin.get(3), Some(&Value::from("glow")));
}

#[test]
fn from_reader_ignores_unknown_flag_bits() {
    assert_eq!(read(&file(0, 0xc000, &[])).unwrap(), Inibin::new());
}

#[test]
fn from_reader_stops_after_string_data() {
    let mut bytes = write(&every_kind());
    bytes.extend_from_slice(b"rest");

    let mut reader = &bytes[..];
    let inibin = Inibin::from_reader(&mut reader).unwrap();

    assert_eq!(inibin, every_kind());
    assert_eq!(reader, b"rest");
}

#[test]
fn from_reader_fails_on_unsupported_version() {
    for version in [0, 3] {
        let result = read(&[version, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert!(matches!(result, Err(Error::UnsupportedVersion(v)) if v == version));
    }
}

/// Returns the bytes of a version 1 file with `entries` and `string_data`. Each entry is a key
/// and a string offset.
fn file_v1(entries: &[(u32, u32)], string_data: &[u8]) -> Vec<u8> {
    let mut bytes = vec![1, 0, 0, 0];
    bytes.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(string_data.len() as u32).to_le_bytes());
    for (key, offset) in entries {
        bytes.extend_from_slice(&key.to_le_bytes());
        bytes.extend_from_slice(&offset.to_le_bytes());
    }
    bytes.extend_from_slice(string_data);
    bytes
}

#[test]
fn from_reader_reads_version_1_file() {
    let bytes = file_v1(&[(0xbb, 8), (0xaa, 0), (0xcc, 8)], b"1 0.5 0\x00450\0");

    let inibin = read(&bytes).unwrap();

    assert_eq!(inibin.len(), 3);
    assert_eq!(inibin.get(0xaa), Some(&Value::from("1 0.5 0")));
    assert_eq!(inibin.get(0xbb), Some(&Value::from("450")));
    assert_eq!(inibin.get(0xcc), Some(&Value::from("450")));
    assert_eq!(inibin.get_vec3(0xaa), Some(vec3(1.0, 0.5, 0.0)));
    assert_eq!(inibin.get_i32(0xbb), Some(450));
}

#[test]
fn from_reader_reads_empty_version_1_file() {
    assert_eq!(read(&file_v1(&[], b"")).unwrap(), Inibin::new());
}

#[test]
fn from_reader_stops_after_string_data_of_version_1_file() {
    let mut bytes = file_v1(&[(0xaa, 0)], b"glow\0");
    bytes.extend_from_slice(b"rest");

    let mut reader = &bytes[..];
    let inibin = Inibin::from_reader(&mut reader).unwrap();

    assert_eq!(inibin, Inibin::from_iter([(0xaa, "glow")]));
    assert_eq!(reader, b"rest");
}

#[test]
fn from_reader_fails_on_duplicate_key_in_version_1_file() {
    let result = read(&file_v1(&[(7, 0), (7, 0)], b"a\0"));
    assert!(matches!(result, Err(Error::DuplicateKey(IniHash(7)))));
}

#[test]
fn from_reader_fails_on_string_offset_out_of_bounds_in_version_1_file() {
    let result = read(&file_v1(&[(7, 70_000)], b"a\0"));
    assert!(matches!(
        result,
        Err(Error::StringOffsetOutOfBounds {
            key: IniHash(7),
            offset: 70_000,
            len: 2
        })
    ));
}

#[test]
fn from_reader_fails_on_truncated_version_1_file() {
    let bytes = file_v1(&[(0xaa, 0), (0xbb, 5)], b"glow\0core\0");
    for len in 0..bytes.len() {
        assert!(matches!(read(&bytes[..len]), Err(Error::Io(_))), "{len}");
    }
}

#[test]
fn to_writer_writes_version_1_file_as_version_2() {
    let inibin = read(&file_v1(&[(0xbb, 5), (0xaa, 0)], b"glow\0core\0")).unwrap();

    let bytes = write(&inibin);

    assert_eq!(bytes[0], 2);
    assert_eq!(bytes[3..5], ValueKind::String.flag().to_le_bytes());
    assert_eq!(read(&bytes).unwrap(), inibin);
}

#[test]
fn from_reader_fails_on_duplicate_key_in_one_set() {
    let mut body = Vec::new();
    body.extend_from_slice(&2u16.to_le_bytes());
    body.extend_from_slice(&7u32.to_le_bytes());
    body.extend_from_slice(&7u32.to_le_bytes());
    body.extend_from_slice(&[1, 2]);

    let result = read(&file(0, ValueKind::U8.flag(), &body));

    assert!(matches!(result, Err(Error::DuplicateKey(IniHash(7)))));
}

#[test]
fn from_reader_fails_on_duplicate_key_in_two_sets() {
    let flags = ValueKind::U8.flag() | ValueKind::String.flag();
    let mut body = Vec::new();
    body.extend_from_slice(&1u16.to_le_bytes());
    body.extend_from_slice(&7u32.to_le_bytes());
    body.push(1);
    body.extend_from_slice(&1u16.to_le_bytes());
    body.extend_from_slice(&7u32.to_le_bytes());
    body.extend_from_slice(&0u16.to_le_bytes());
    body.extend_from_slice(b"a\0");

    let result = read(&file(2, flags, &body));

    assert!(matches!(result, Err(Error::DuplicateKey(IniHash(7)))));
}

#[test]
fn from_reader_fails_on_string_offset_out_of_bounds() {
    let result = read(&one_string(3, b"a\0"));
    assert!(matches!(
        result,
        Err(Error::StringOffsetOutOfBounds {
            key: IniHash(7),
            offset: 3,
            len: 2
        })
    ));
}

#[test]
fn from_reader_fails_on_unterminated_string() {
    let result = read(&one_string(0, b"ab"));
    assert!(matches!(
        result,
        Err(Error::UnterminatedString { key: IniHash(7) })
    ));
}

#[test]
fn from_reader_fails_on_invalid_utf8_string() {
    let result = read(&one_string(0, b"\xff\0"));
    assert!(matches!(
        result,
        Err(Error::InvalidUtf8 {
            key: IniHash(7),
            ..
        })
    ));
}

#[test]
fn from_reader_fails_on_truncated_file() {
    let bytes = write(&every_kind());
    for len in 0..bytes.len() {
        assert!(matches!(read(&bytes[..len]), Err(Error::Io(_))), "{len}");
    }
}

#[test]
fn to_writer_writes_empty_file() {
    assert_eq!(write(&Inibin::new()), [2, 0, 0, 0, 0]);
}

#[test]
fn to_writer_writes_sets_in_kind_order() {
    let inibin = Inibin::from_iter([
        (0x30, Value::from("b")),
        (0x20, Value::I64(9)),
        (0x10, Value::from("a")),
        (0x40, Value::U8(3)),
    ]);

    let flags = ValueKind::U8.flag() | ValueKind::String.flag() | ValueKind::I64.flag();
    let mut body = Vec::new();
    body.extend_from_slice(&1u16.to_le_bytes());
    body.extend_from_slice(&0x40u32.to_le_bytes());
    body.push(3);
    body.extend_from_slice(&2u16.to_le_bytes());
    body.extend_from_slice(&0x10u32.to_le_bytes());
    body.extend_from_slice(&0x30u32.to_le_bytes());
    body.extend_from_slice(&0u16.to_le_bytes());
    body.extend_from_slice(&2u16.to_le_bytes());
    body.extend_from_slice(&1u16.to_le_bytes());
    body.extend_from_slice(&0x20u32.to_le_bytes());
    body.extend_from_slice(&9i64.to_le_bytes());
    body.extend_from_slice(b"a\0b\0");
    assert_eq!(write(&inibin), file(4, flags, &body));
}

#[test]
fn to_writer_packs_bool_from_lowest_bit_first() {
    let inibin: Inibin = (0..10u32).map(|key| (key, key % 3 == 0)).collect();

    let bytes = write(&inibin);

    assert_eq!(bytes[bytes.len() - 2..], [0b0100_1001, 0b0000_0010]);
    assert_eq!(read(&bytes).unwrap(), inibin);
}

#[test]
fn to_writer_stores_equal_strings_once() {
    let inibin = Inibin::from_iter([(1, "glow"), (2, "core"), (3, "glow"), (4, "")]);

    let bytes = write(&inibin);

    assert!(bytes.ends_with(b"\x09\0glow\0core\0"));
    assert_eq!(bytes[1..3], 10u16.to_le_bytes());
    assert_eq!(read(&bytes).unwrap(), inibin);
}

#[test]
fn to_writer_stores_suffix_inside_longer_string() {
    let inibin = Inibin::from_iter([
        (1, "Akali"),
        (2, "tips_Akali"),
        (3, "game_character_tips_Akali"),
        (4, "Akal"),
    ]);

    let bytes = write(&inibin);

    assert!(bytes.ends_with(b"game_character_tips_Akali\0Akal\0"));
    let offsets = &bytes[bytes.len() - 31 - 8..bytes.len() - 31];
    assert_eq!(offsets, [20, 0, 15, 0, 0, 0, 26, 0]);
    assert_eq!(read(&bytes).unwrap(), inibin);
}

#[test]
fn layout_strings_keeps_input_order_of_stored_strings() {
    let (data, offsets) = layout_strings(&["b", "ab", "c", "b", "cab", ""]);
    assert_eq!(data, b"c\0cab\0");
    assert_eq!(offsets, [4, 3, 0, 4, 2, 5]);
}

#[test]
fn to_writer_fails_on_nul_in_string() {
    let inibin = Inibin::from_iter([(7, "a\0b")]);
    let result = inibin.to_writer(&mut Vec::new());
    assert!(matches!(
        result,
        Err(Error::NulInString { key: IniHash(7) })
    ));
}

#[test]
fn to_writer_fails_on_too_many_values_of_one_kind() {
    let inibin: Inibin = (0..=u32::from(u16::MAX)).map(|key| (key, 2u8)).collect();
    let result = inibin.to_writer(&mut Vec::new());
    assert!(matches!(
        result,
        Err(Error::TooManyValues {
            kind: ValueKind::U8,
            count: 65536
        })
    ));
}

#[test]
fn to_writer_fails_on_string_data_too_long() {
    let inibin = Inibin::from_iter([(1, "a".repeat(40_000)), (2, "b".repeat(40_000))]);
    let result = inibin.to_writer(&mut Vec::new());
    assert!(matches!(
        result,
        Err(Error::StringDataTooLong { len: 80_002 })
    ));
}

#[test]
fn to_writer_round_trips_every_kind() {
    let inibin = every_kind();
    let bytes = write(&inibin);
    assert_eq!(bytes[3..5], 0x3fffu16.to_le_bytes());
    assert_eq!(read(&bytes).unwrap(), inibin);
}

#[test]
fn insert_replaces_value_of_other_kind() {
    let mut inibin = Inibin::new();
    assert_eq!(inibin.insert(7, 42), None);
    assert_eq!(inibin.insert(7, "text"), Some(Value::I32(42)));
    assert_eq!(inibin.len(), 1);
    assert_eq!(inibin.get(7), Some(&Value::from("text")));
}

#[test]
fn get_accepts_ini_hash_and_str_key() {
    let mut inibin = Inibin::new();
    inibin.insert(0x0616933a, "glow");
    assert_eq!(
        inibin.get(IniHash::new("System", "GroupPart1")),
        Some(&Value::from("glow"))
    );
    assert_eq!(inibin.get("system*grouppart1"), Some(&Value::from("glow")));
}

#[test]
fn retain_removes_rejected_values() {
    let mut inibin = every_kind();
    inibin.retain(|_, value| value.kind() == ValueKind::String);
    assert_eq!(inibin, Inibin::from_iter([(13, "glow")]));
}

#[test]
fn keys_iterates_in_ascending_order() {
    let inibin = Inibin::from_iter([(3, 1u8), (1, 2u8), (2, 3u8)]);
    let keys: Vec<u32> = inibin.keys().map(|key| key.0).collect();
    assert_eq!(keys, [1, 2, 3]);
}

#[test]
fn serde_round_trips_every_kind() {
    let inibin = every_kind();
    let json = serde_json::to_string(&inibin).unwrap();
    assert_eq!(serde_json::from_str::<Inibin>(&json).unwrap(), inibin);
}
