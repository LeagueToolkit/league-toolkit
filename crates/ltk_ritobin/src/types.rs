//! Type name mappings for ritobin format.

use ltk_meta::BinPropertyKind;
use std::str::FromStr;

/// Maps a ritobin type name string to a BinPropertyKind.
pub fn type_name_to_kind(name: &str) -> Option<BinPropertyKind> {
    match name {
        "none" => Some(BinPropertyKind::None),
        "bool" => Some(BinPropertyKind::Bool),
        "i8" => Some(BinPropertyKind::I8),
        "u8" => Some(BinPropertyKind::U8),
        "i16" => Some(BinPropertyKind::I16),
        "u16" => Some(BinPropertyKind::U16),
        "i32" => Some(BinPropertyKind::I32),
        "u32" => Some(BinPropertyKind::U32),
        "i64" => Some(BinPropertyKind::I64),
        "u64" => Some(BinPropertyKind::U64),
        "f32" => Some(BinPropertyKind::F32),
        "vec2" => Some(BinPropertyKind::Vector2),
        "vec3" => Some(BinPropertyKind::Vector3),
        "vec4" => Some(BinPropertyKind::Vector4),
        "mtx44" => Some(BinPropertyKind::Matrix44),
        "rgba" => Some(BinPropertyKind::Color),
        "string" => Some(BinPropertyKind::String),
        "hash" => Some(BinPropertyKind::Hash),
        "file" => Some(BinPropertyKind::WadChunkLink),
        "list" => Some(BinPropertyKind::Container),
        "list2" => Some(BinPropertyKind::UnorderedContainer),
        "pointer" => Some(BinPropertyKind::Struct),
        "embed" => Some(BinPropertyKind::Embedded),
        "link" => Some(BinPropertyKind::ObjectLink),
        "option" => Some(BinPropertyKind::Optional),
        "map" => Some(BinPropertyKind::Map),
        "flag" => Some(BinPropertyKind::BitBool),
        _ => None,
    }
}

/// Maps a BinPropertyKind to its ritobin type name string.
pub fn kind_to_type_name(kind: BinPropertyKind) -> &'static str {
    match kind {
        BinPropertyKind::None => "none",
        BinPropertyKind::Bool => "bool",
        BinPropertyKind::I8 => "i8",
        BinPropertyKind::U8 => "u8",
        BinPropertyKind::I16 => "i16",
        BinPropertyKind::U16 => "u16",
        BinPropertyKind::I32 => "i32",
        BinPropertyKind::U32 => "u32",
        BinPropertyKind::I64 => "i64",
        BinPropertyKind::U64 => "u64",
        BinPropertyKind::F32 => "f32",
        BinPropertyKind::Vector2 => "vec2",
        BinPropertyKind::Vector3 => "vec3",
        BinPropertyKind::Vector4 => "vec4",
        BinPropertyKind::Matrix44 => "mtx44",
        BinPropertyKind::Color => "rgba",
        BinPropertyKind::String => "string",
        BinPropertyKind::Hash => "hash",
        BinPropertyKind::WadChunkLink => "file",
        BinPropertyKind::Container => "list",
        BinPropertyKind::UnorderedContainer => "list2",
        BinPropertyKind::Struct => "pointer",
        BinPropertyKind::Embedded => "embed",
        BinPropertyKind::ObjectLink => "link",
        BinPropertyKind::Optional => "option",
        BinPropertyKind::Map => "map",
        BinPropertyKind::BitBool => "flag",
    }
}

/// Ritobin type representation for parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RitobinType {
    pub kind: BinPropertyKind,
    pub inner_kind: Option<BinPropertyKind>,
    pub value_kind: Option<BinPropertyKind>,
}

impl RitobinType {
    pub fn simple(kind: BinPropertyKind) -> Self {
        Self {
            kind,
            inner_kind: None,
            value_kind: None,
        }
    }

    pub fn container(kind: BinPropertyKind, inner: BinPropertyKind) -> Self {
        Self {
            kind,
            inner_kind: Some(inner),
            value_kind: None,
        }
    }

    pub fn map(key: BinPropertyKind, value: BinPropertyKind) -> Self {
        Self {
            kind: BinPropertyKind::Map,
            inner_kind: Some(key),
            value_kind: Some(value),
        }
    }
}

impl FromStr for RitobinType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        type_name_to_kind(s).map(RitobinType::simple).ok_or(())
    }
}
