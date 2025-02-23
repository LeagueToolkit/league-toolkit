use lazy_static::lazy_static;

use super::LeagueFileKind;

pub static LEAGUE_FILE_MAGIC_BYTES: &[LeagueFilePattern] = &[
    LeagueFilePattern::from_bytes(crate::core::mesh::MAGIC, LeagueFileKind::StaticMeshBinary),
    LeagueFilePattern::from_bytes(b"r3d2sklt", LeagueFileKind::Skeleton),
    LeagueFilePattern::from_bytes(b"r3d2ammd", LeagueFileKind::Animation),
    LeagueFilePattern::from_bytes(b"r3d2canm", LeagueFileKind::Animation),
    LeagueFilePattern::from_fn(
        |data| u32::from_le_bytes(data[4..8].try_into().unwrap()) == 1,
        8,
        LeagueFileKind::WwisePackage,
    ),
    LeagueFilePattern::from_fn(|data| &data[1..4] == b"PNG", 4, LeagueFileKind::Png),
    LeagueFilePattern::from_bytes(b"DDS ", LeagueFileKind::TextureDds),
    LeagueFilePattern::from_bytes(&[0x33, 0x22, 0x11, 0x00], LeagueFileKind::SimpleSkin),
    LeagueFilePattern::from_bytes(b"PROP", LeagueFileKind::PropertyBin),
    LeagueFilePattern::from_bytes(b"BKHD", LeagueFileKind::WwiseBank),
    LeagueFilePattern::from_bytes(b"WGEO", LeagueFileKind::WorldGeometry),
    LeagueFilePattern::from_bytes(b"OEGM", LeagueFileKind::MapGeometry),
    LeagueFilePattern::from_bytes(b"[Obj", LeagueFileKind::StaticMeshAscii),
    LeagueFilePattern::from_fn(|data| &data[1..5] == b"LuaQ", 5, LeagueFileKind::LuaObj),
    LeagueFilePattern::from_bytes(b"PreLoad", LeagueFileKind::Preload),
    LeagueFilePattern::from_fn(
        |data| u32::from_le_bytes(data[..4].try_into().unwrap()) == 3,
        4,
        LeagueFileKind::LightGrid,
    ),
    LeagueFilePattern::from_bytes(b"RST", LeagueFileKind::RiotStringTable),
    LeagueFilePattern::from_bytes(b"PTCH", LeagueFileKind::PropertyBinOverride),
    LeagueFilePattern::from_fn(
        |data| ((u32::from_le_bytes(data[..4].try_into().unwrap()) & 0x00FFFFFF) == 0x00FFD8FF),
        3,
        LeagueFileKind::Jpeg,
    ),
    LeagueFilePattern::from_fn(
        |data| u32::from_le_bytes(data[4..8].try_into().unwrap()) == 0x22FD4FC3,
        8,
        LeagueFileKind::Skeleton,
    ),
    LeagueFilePattern::from_bytes(b"TEX\0", LeagueFileKind::Texture),
    LeagueFilePattern::from_bytes(b"<svg", LeagueFileKind::Svg),
];

lazy_static! {
    /// The length of the largest possible file type magic, in bytes.
    pub static ref MAX_MAGIC_SIZE: usize = {
        LEAGUE_FILE_MAGIC_BYTES
            .iter()
            .map(|p| p.min_length)
            .max()
            .unwrap()
    };
}

pub enum LeagueFilePatternKind {
    Bytes(&'static [u8]),
    Fn(fn(&[u8]) -> bool),
}

pub struct LeagueFilePattern {
    pub pattern: LeagueFilePatternKind,
    pub min_length: usize,
    pub kind: LeagueFileKind,
}

impl LeagueFilePattern {
    const fn from_bytes(bytes: &'static [u8], kind: LeagueFileKind) -> Self {
        Self {
            pattern: LeagueFilePatternKind::Bytes(bytes),
            min_length: bytes.len(),
            kind,
        }
    }

    const fn from_fn(f: fn(&[u8]) -> bool, min_length: usize, kind: LeagueFileKind) -> Self {
        Self {
            pattern: LeagueFilePatternKind::Fn(f),
            min_length,
            kind,
        }
    }

    pub fn matches(&self, data: &[u8]) -> bool {
        data.len() >= self.min_length
            && match self.pattern {
                LeagueFilePatternKind::Bytes(bytes) => &data[..bytes.len()] == bytes,
                LeagueFilePatternKind::Fn(f) => f(data),
            }
    }
}
