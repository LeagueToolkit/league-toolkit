//! Type name mappings for ritobin format.

use ltk_meta::property::Kind;
use std::str::FromStr;

/// Maps a ritobin type name string to a BinPropertyKind.
pub fn type_name_to_kind(name: &str) -> Option<Kind> {
    match name {
        "none" => Some(Kind::None),
        "bool" => Some(Kind::Bool),
        "i8" => Some(Kind::I8),
        "u8" => Some(Kind::U8),
        "i16" => Some(Kind::I16),
        "u16" => Some(Kind::U16),
        "i32" => Some(Kind::I32),
        "u32" => Some(Kind::U32),
        "i64" => Some(Kind::I64),
        "u64" => Some(Kind::U64),
        "f32" => Some(Kind::F32),
        "vec2" => Some(Kind::Vector2),
        "vec3" => Some(Kind::Vector3),
        "vec4" => Some(Kind::Vector4),
        "mtx44" => Some(Kind::Matrix44),
        "rgba" => Some(Kind::Color),
        "string" => Some(Kind::String),
        "hash" => Some(Kind::Hash),
        "file" => Some(Kind::WadChunkLink),
        "list" => Some(Kind::Container),
        "list2" => Some(Kind::UnorderedContainer),
        "pointer" => Some(Kind::Struct),
        "embed" => Some(Kind::Embedded),
        "link" => Some(Kind::ObjectLink),
        "option" => Some(Kind::Optional),
        "map" => Some(Kind::Map),
        "flag" => Some(Kind::BitBool),
        _ => None,
    }
}

/// Maps a BinPropertyKind to its ritobin type name string.
pub fn kind_to_type_name(kind: Kind) -> &'static str {
    match kind {
        Kind::None => "none",
        Kind::Bool => "bool",
        Kind::I8 => "i8",
        Kind::U8 => "u8",
        Kind::I16 => "i16",
        Kind::U16 => "u16",
        Kind::I32 => "i32",
        Kind::U32 => "u32",
        Kind::I64 => "i64",
        Kind::U64 => "u64",
        Kind::F32 => "f32",
        Kind::Vector2 => "vec2",
        Kind::Vector3 => "vec3",
        Kind::Vector4 => "vec4",
        Kind::Matrix44 => "mtx44",
        Kind::Color => "rgba",
        Kind::String => "string",
        Kind::Hash => "hash",
        Kind::WadChunkLink => "file",
        Kind::Container => "list",
        Kind::UnorderedContainer => "list2",
        Kind::Struct => "pointer",
        Kind::Embedded => "embed",
        Kind::ObjectLink => "link",
        Kind::Optional => "option",
        Kind::Map => "map",
        Kind::BitBool => "flag",
    }
}

/// Ritobin type representation for parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RitobinType {
    pub kind: Kind,
    pub inner_kind: Option<Kind>,
    pub value_kind: Option<Kind>,
}

impl RitobinType {
    pub fn simple(kind: Kind) -> Self {
        Self {
            kind,
            inner_kind: None,
            value_kind: None,
        }
    }

    pub fn container(kind: Kind, inner: Kind) -> Self {
        Self {
            kind,
            inner_kind: Some(inner),
            value_kind: None,
        }
    }

    pub fn map(key: Kind, value: Kind) -> Self {
        Self {
            kind: Kind::Map,
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
