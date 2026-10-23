use std::fmt::Display;

use crate::{BadContainerType, BadScalarType, ContainerType, ScalarType};

/// Ritobin type representation for parsing.
///
/// The type `hash[width,algorithm]` gives the stored width and the hash function of a `hash`,
/// for example `hash[u64,xxh3]`. The type `hash` is `hash[u32,fnv1a]`. The type `hash[width]`
/// gives the width alone; see [`crate::HashType`].
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum RitoType {
    /// A type that is not a container.
    Scalar(ScalarType),
    /// A `list`, `list2`, `option` or `map`, with its subtypes.
    Container(ContainerType),
}

impl PartialEq<ScalarType> for RitoType {
    fn eq(&self, other: &ScalarType) -> bool {
        match self {
            RitoType::Scalar(scalar) => scalar == other,
            RitoType::Container(_) => false,
        }
    }
}

impl PartialEq<ContainerType> for RitoType {
    fn eq(&self, other: &ContainerType) -> bool {
        match self {
            RitoType::Scalar(_) => false,
            RitoType::Container(container_type) => other == container_type,
        }
    }
}

/// The error for a type that cannot be built from a base kind and its subtypes.
#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadRitoType {
    /// No subtypes are given, and the base kind is a container kind.
    #[error("No subtypes given, but {0}")]
    BadScalarType(#[from] BadScalarType),
    /// Subtypes are given, and they do not build a container type.
    #[error("Subtypes given, but {0}")]
    BadContainerType(#[from] BadContainerType),
}

impl From<ScalarType> for RitoType {
    fn from(value: ScalarType) -> Self {
        Self::Scalar(value)
    }
}
impl From<ContainerType> for RitoType {
    fn from(value: ContainerType) -> Self {
        Self::Container(value)
    }
}

impl Display for RitoType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RitoType::Scalar(scalar) => scalar.fmt(f),
            RitoType::Container(container) => container.fmt(f),
        }
    }
}
