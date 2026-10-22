use std::fmt::Display;

use crate::{BadContainerType, BadScalarType, ContainerType, ScalarType};

/// Ritobin type representation for parsing.
///
/// The type `hash[width,algorithm]` gives the stored width and the hash function of a `hash`,
/// for example `hash[u64,xxh3]`. The type `hash` is `hash[u32,fnv1a]`. The type `hash[width]`
/// gives the width alone; see [`HashType`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RitoType {
    Scalar(ScalarType),
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

#[derive(thiserror::Error, Debug)]
pub enum BadRitoType {
    #[error("No subtypes given, but {0}")]
    BadScalarType(#[from] BadScalarType),
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
