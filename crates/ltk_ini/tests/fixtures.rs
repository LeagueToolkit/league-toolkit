//! Tests `Inibin` against the shipped files in `tests/fixtures/`.

use std::path::Path;

use ltk_ini::{IniHash, Inibin, Value, ValueKind};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

fn read_fixture(name: &str) -> Vec<u8> {
    let path = Path::new(FIXTURES).join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

fn write(inibin: &Inibin) -> Vec<u8> {
    let mut bytes = Vec::new();
    inibin.to_writer(&mut bytes).unwrap();
    bytes
}

fn count(inibin: &Inibin, kind: ValueKind) -> usize {
    inibin.values().filter(|v| v.kind() == kind).count()
}

/// The file name of a shipped particle system. The file has values of every storage type except
/// `I64`.
const TROYBIN: &str = "global_item_travelsizeelixirofiron_buf.troybin";

/// The file name of a shipped summoner spell list of a bot. The file has 12 string values and 2
/// distinct strings.
const INIBIN: &str = "aatroxbotsummonerspellsclassic.inibin";

#[test]
fn from_reader_reads_every_value_of_troybin() {
    let inibin = Inibin::from_reader(&mut read_fixture(TROYBIN).as_slice()).unwrap();

    let counts = ValueKind::ALL.map(|kind| count(&inibin, kind));
    assert_eq!(counts, [1, 13, 16, 3, 11, 12, 17, 8, 36, 21, 15, 4, 41, 0]);
    assert_eq!(inibin.len(), 198);

    assert_eq!(inibin.get(0xa8310708), Some(&Value::I32(100_000)));
    assert_eq!(inibin.get(0x63cb7a49), Some(&Value::F32(1.25)));
    assert_eq!(inibin.get(0x89248720), Some(&Value::Fixed(15)));
    assert_eq!(inibin.get(0x41f79618), Some(&Value::I16(-50)));
    assert_eq!(inibin.get(0x7f3c4234), Some(&Value::U8(20)));
    assert_eq!(inibin.get(0x354ff09f), Some(&Value::U1(false)));
    assert_eq!(inibin.get(0x3f1cf037), Some(&Value::FixedVec3([0, 200, 0])));
    assert_eq!(
        inibin.get(0xb02da149),
        Some(&Value::Vec3(glam::vec3(40.0, 80.0, 12.0)))
    );
    assert_eq!(inibin.get(0x36b464c4), Some(&Value::FixedVec2([20, 10])));
    assert_eq!(
        inibin.get(0x4c09d5eb),
        Some(&Value::Vec2(glam::vec2(0.5, -45.0)))
    );
    assert_eq!(
        inibin.get(0x47a85dd8),
        Some(&Value::FixedVec4([10, 10, 10, 5]))
    );
    assert_eq!(
        inibin.get(0x7fd3ca74),
        Some(&Value::Vec4(glam::vec4(0.75, 3.0, 5.0, 0.0)))
    );
}

#[test]
fn section_resolves_troybin_emitters_by_name() {
    let inibin = Inibin::from_reader(&mut read_fixture(TROYBIN).as_slice()).unwrap();

    let system = inibin.section("System");
    let emitters: Vec<&str> = (1..)
        .map_while(|index| system.get_str(&format!("GroupPart{index}")))
        .collect();
    assert_eq!(system.key("GroupPart1"), IniHash(0x0616933a));

    assert_eq!(
        emitters,
        [
            "glow",
            "core",
            "SoftBeams",
            "DarkGlow",
            "SoftBeamsBurst",
            "glowburst"
        ]
    );
}

#[test]
fn get_f32_converts_f32_and_fixed_in_troybin() {
    let inibin = Inibin::from_reader(&mut read_fixture(TROYBIN).as_slice()).unwrap();

    let glow = inibin.section("glow");
    let dark_glow = inibin.section("DarkGlow");

    assert_eq!(glow.get("p-life"), Some(&Value::F32(1.55)));
    assert_eq!(dark_glow.get("p-life"), Some(&Value::Fixed(15)));
    assert_eq!(glow.get_f32("p-life"), Some(1.55));
    assert_eq!(dark_glow.get_f32("p-life"), Some(1.5));
    assert_eq!(
        glow.get_str("p-texture"),
        Some("DATA/Particles/Global_Item_ElixirOfIron_Glow.dds")
    );
}

#[test]
fn to_writer_keeps_troybin_bytes_before_string_offsets() {
    let bytes = read_fixture(TROYBIN);
    let inibin = Inibin::from_reader(&mut bytes.as_slice()).unwrap();

    let written = write(&inibin);

    // The ini compiler orders the string data by the ini text. `to_writer` orders the string data
    // by key. The test compares only the bytes before the string offsets.
    let strings = count(&inibin, ValueKind::String);
    let string_data_len = usize::from(u16::from_le_bytes([bytes[1], bytes[2]]));
    let offsets_start = bytes.len() - string_data_len - 2 * strings;
    assert_eq!(written.len(), bytes.len());
    assert_eq!(written[..offsets_start], bytes[..offsets_start]);
    assert_eq!(
        Inibin::from_reader(&mut written.as_slice()).unwrap(),
        inibin
    );
}

#[test]
fn to_writer_writes_inibin_byte_for_byte() {
    let bytes = read_fixture(INIBIN);
    let inibin = Inibin::from_reader(&mut bytes.as_slice()).unwrap();

    assert_eq!(inibin.len(), 12);
    assert!(inibin
        .values()
        .all(|v| matches!(v.as_str(), Some("SummonerDot" | "SummonerExhaust"))));
    assert_eq!(write(&inibin), bytes);
}
