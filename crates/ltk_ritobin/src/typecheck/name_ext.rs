use ltk_meta::BinPropertyKind;

pub trait RitobinName: Sized {
    fn from_ritobin_name(name: &str) -> Option<Self>;
    fn to_ritobin_name(self) -> &'static str;
}

impl RitobinName for BinPropertyKind {
    fn from_ritobin_name(name: &str) -> Option<Self> {
        match name {
            "none" => Some(Self::None),
            "bool" => Some(Self::Bool),
            "i8" => Some(Self::I8),
            "u8" => Some(Self::U8),
            "i16" => Some(Self::I16),
            "u16" => Some(Self::U16),
            "i32" => Some(Self::I32),
            "u32" => Some(Self::U32),
            "i64" => Some(Self::I64),
            "u64" => Some(Self::U64),
            "f32" => Some(Self::F32),
            "vec2" => Some(Self::Vector2),
            "vec3" => Some(Self::Vector3),
            "vec4" => Some(Self::Vector4),
            "mtx44" => Some(Self::Matrix44),
            "rgba" => Some(Self::Color),
            "string" => Some(Self::String),
            "hash" => Some(Self::Hash),
            "file" => Some(Self::WadChunkLink),
            "list" => Some(Self::Container),
            "list2" => Some(Self::UnorderedContainer),
            "pointer" => Some(Self::Struct),
            "embed" => Some(Self::Embedded),
            "link" => Some(Self::ObjectLink),
            "option" => Some(Self::Optional),
            "map" => Some(Self::Map),
            "flag" => Some(Self::BitBool),
            _ => None,
        }
    }

    fn to_ritobin_name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Bool => "bool",
            Self::I8 => "i8",
            Self::U8 => "u8",
            Self::I16 => "i16",
            Self::U16 => "u16",
            Self::I32 => "i32",
            Self::U32 => "u32",
            Self::I64 => "i64",
            Self::U64 => "u64",
            Self::F32 => "f32",
            Self::Vector2 => "vec2",
            Self::Vector3 => "vec3",
            Self::Vector4 => "vec4",
            Self::Matrix44 => "mtx44",
            Self::Color => "rgba",
            Self::String => "string",
            Self::Hash => "hash",
            Self::WadChunkLink => "file",
            Self::Container => "list",
            Self::UnorderedContainer => "list2",
            Self::Struct => "pointer",
            Self::Embedded => "embed",
            Self::ObjectLink => "link",
            Self::Optional => "option",
            Self::Map => "map",
            Self::BitBool => "flag",
        }
    }
}
