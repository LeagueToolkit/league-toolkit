use std::fmt::Display;

use ltk_meta::PropertyKind;

use crate::{HashType, RitoType, RitobinName as _};

/// A ritobin type that is not a container.
///
/// A container holds scalars only. Each variant names the [`PropertyKind`] of the same name.
#[derive(Debug, Default, Clone, Copy, Hash, PartialEq, Eq)]
pub enum ScalarType {
    /// `none`
    #[default]
    None,
    /// `bool`
    Bool,
    /// `i8`
    I8,
    /// `u8`
    U8,
    /// `i16`
    I16,
    /// `u16`
    U16,
    /// `i32`
    I32,
    /// `u32`
    U32,
    /// `i64`
    I64,
    /// `u64`
    U64,
    /// `f32`
    F32,
    /// `vec2`
    Vector2,
    /// `vec3`
    Vector3,
    /// `vec4`
    Vector4,
    /// `mtx44`
    Matrix44,
    /// `rgba`
    Color,
    /// `string`
    String,
    /// `file`
    WadChunkLink,
    /// `pointer`
    Struct,
    /// `embed`
    Embedded,
    /// `link`
    ObjectLink,
    /// `flag`
    BitBool,
    /// `hash`, with its width and hash function.
    Hash(HashType),
}

impl ScalarType {
    /// Returns the type with [`HashType::DEFAULT`] if it is `hash`.
    ///
    /// Any other type is returned unchanged.
    pub const fn with_default_hash_type(self) -> Self {
        self.with_hash_type(HashType::DEFAULT)
    }
    /// Returns the type with `hash_type` if it is `hash`.
    ///
    /// Any other type is returned unchanged.
    pub const fn with_hash_type(self, hash_type: HashType) -> Self {
        match self {
            ScalarType::Hash(_) => Self::Hash(hash_type),
            _ => self,
        }
    }

    /// Returns the hash type of a `hash`. Returns `None` for any other type.
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

/// The error for a [`PropertyKind`] that is a container kind, where a scalar is required.
#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
#[error("{0:?} is not a scalar type")]
pub struct BadScalarType(pub PropertyKind);
impl ScalarType {
    /// Returns the [`PropertyKind`] of the type.
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
    /// Returns the scalar type of `kind`. A `hash` holds [`HashType::DEFAULT`].
    ///
    /// # Errors
    /// Returns [`BadScalarType`] if `kind` is a container kind.
    pub const fn try_from_kind(kind: PropertyKind) -> Result<Self, BadScalarType> {
        match RitoType::from_property_kind(kind) {
            RitoType::Scalar(scalar) => Ok(scalar),
            RitoType::Container(_) => Err(BadScalarType(kind)),
        }
    }
}
