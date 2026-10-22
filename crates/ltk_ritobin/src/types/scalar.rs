use std::fmt::Display;

use ltk_meta::PropertyKind;

use crate::{HashType, RitobinName as _};

#[derive(Debug, Default, Clone, Copy, Hash, PartialEq, Eq)]
pub enum ScalarType {
    #[default]
    None,
    Bool,
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F32,
    Vector2,
    Vector3,
    Vector4,
    Matrix44,
    Color,
    String,
    WadChunkLink,
    Struct,
    Embedded,
    ObjectLink,
    BitBool,
    Hash(HashType),
}

impl ScalarType {
    /// Return the same type, but with the [`Self::Hash`] changed to use [`HashType::default`]
    pub const fn with_default_hash_type(self) -> Self {
        self.with_hash_type(HashType::DEFAULT)
    }
    /// Return the same type, but with the [`Self::Hash`] changed to use the given [`HashType`]
    pub const fn with_hash_type(self, hash_type: HashType) -> Self {
        match self {
            ScalarType::Hash(_) => Self::Hash(hash_type),
            _ => self,
        }
    }

    /// The hash type, if we are [`Self::Hash`]
    pub const fn hash_type(&self) -> Option<HashType> {
        match self {
            ScalarType::Hash(hash_type) => Some(*hash_type),
            _ => None,
        }
    }
}

impl Display for ScalarType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hash(hash_type) => hash_type.fmt(f),
            _ => f.write_str(self.kind().to_rito_name()),
        }
    }
}

impl TryFrom<PropertyKind> for ScalarType {
    type Error = BadScalarType;

    fn try_from(kind: PropertyKind) -> Result<Self, Self::Error> {
        Self::try_from_kind(kind)
    }
}
impl From<ScalarType> for PropertyKind {
    fn from(value: ScalarType) -> Self {
        value.kind()
    }
}

impl From<HashType> for ScalarType {
    fn from(value: HashType) -> Self {
        Self::Hash(value)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{0:?} is not a scalar type")]
pub struct BadScalarType(pub PropertyKind);
impl ScalarType {
    pub const fn kind(self) -> PropertyKind {
        use PropertyKind as K;
        match self {
            ScalarType::None => K::None,
            ScalarType::Bool => K::Bool,
            ScalarType::I8 => K::I8,
            ScalarType::U8 => K::U8,
            ScalarType::I16 => K::I16,
            ScalarType::U16 => K::U16,
            ScalarType::I32 => K::I32,
            ScalarType::U32 => K::U32,
            ScalarType::I64 => K::I64,
            ScalarType::U64 => K::U64,
            ScalarType::F32 => K::F32,
            ScalarType::Vector2 => K::Vector2,
            ScalarType::Vector3 => K::Vector3,
            ScalarType::Vector4 => K::Vector4,
            ScalarType::Matrix44 => K::Matrix44,
            ScalarType::Color => K::Color,
            ScalarType::String => K::String,
            ScalarType::WadChunkLink => K::WadChunkLink,
            ScalarType::Struct => K::Struct,
            ScalarType::Embedded => K::Embedded,
            ScalarType::ObjectLink => K::ObjectLink,
            ScalarType::BitBool => K::BitBool,
            ScalarType::Hash(_) => K::Hash,
        }
    }
    pub const fn try_from_kind(kind: PropertyKind) -> Result<Self, BadScalarType> {
        use PropertyKind as K;
        Ok(match kind {
            K::None => Self::None,
            K::Bool => Self::Bool,
            K::I8 => Self::I8,
            K::U8 => Self::U8,
            K::I16 => Self::I16,
            K::U16 => Self::U16,
            K::I32 => Self::I32,
            K::U32 => Self::U32,
            K::I64 => Self::I64,
            K::U64 => Self::U64,
            K::F32 => Self::F32,
            K::Vector2 => Self::Vector2,
            K::Vector3 => Self::Vector3,
            K::Vector4 => Self::Vector4,
            K::Matrix44 => Self::Matrix44,
            K::Color => Self::Color,
            K::String => Self::String,
            K::WadChunkLink => Self::WadChunkLink,
            K::Struct => Self::Struct,
            K::Embedded => Self::Embedded,
            K::ObjectLink => Self::ObjectLink,
            K::BitBool => Self::BitBool,
            K::Hash => Self::Hash(HashType::DEFAULT),
            kind => return Err(BadScalarType(kind)),
        })
    }
}
