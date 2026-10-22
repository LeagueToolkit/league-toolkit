use std::fmt::Display;

use ltk_meta::PropertyKind;

use crate::{HashType, RitobinName as _, ScalarType};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum ContainerType {
    Map {
        key: Option<ScalarType>,
        value: Option<ScalarType>,
    },
    Container(Option<ScalarType>),
    UnorderedContainer(Option<ScalarType>),
    Optional(Option<ScalarType>),
}

impl ContainerType {
    /// Return the same type, but with any hash subtypes changed to use the given [`HashType`]s.
    pub fn with_hash_types(self, hash_types: [HashType; 2]) -> Self {
        match self {
            ContainerType::Map { key, value } => Self::Map {
                key: key.map(|k| k.with_hash_type(hash_types[0])),
                value: value.map(|k| k.with_hash_type(hash_types[1])),
            },
            ContainerType::Container(sub) => {
                Self::Container(sub.map(|sub| sub.with_hash_type(hash_types[0])))
            }
            ContainerType::UnorderedContainer(sub) => {
                Self::UnorderedContainer(sub.map(|sub| sub.with_hash_type(hash_types[0])))
            }
            ContainerType::Optional(sub) => {
                Self::Optional(sub.map(|sub| sub.with_hash_type(hash_types[0])))
            }
        }
    }
    /// Return the same type, but with any hash subtypes changed to use [`HashType::default`]
    pub fn with_default_hash_types(self) -> Self {
        self.with_hash_types([HashType::DEFAULT, HashType::DEFAULT])
    }

    pub const fn base_kind(&self) -> PropertyKind {
        use PropertyKind as K;
        match self {
            ContainerType::Map { .. } => K::Map,
            ContainerType::Container(..) => K::Container,
            ContainerType::UnorderedContainer(..) => K::UnorderedContainer,
            ContainerType::Optional(..) => K::Optional,
        }
    }

    /// Get the base kind & subtypes as [`PropertyKind`]s.
    pub fn into_kind_parts(self) -> (PropertyKind, [Option<PropertyKind>; 2]) {
        use PropertyKind as K;
        match self {
            ContainerType::Map { key, value } => (
                K::Map,
                [key.map(ScalarType::kind), value.map(ScalarType::kind)],
            ),
            ContainerType::Container(item) => (K::Container, [item.map(ScalarType::kind), None]),
            ContainerType::UnorderedContainer(item) => {
                (K::UnorderedContainer, [item.map(ScalarType::kind), None])
            }
            ContainerType::Optional(item) => (K::Optional, [item.map(ScalarType::kind), None]),
        }
    }

    /// Returns the `n`th subtype, if present.
    pub const fn subtype(&self, n: usize) -> Option<ScalarType> {
        match self {
            Self::Map { key, value } => match n {
                0 => *key,
                1 => *value,
                _ => None,
            },
            Self::Container(sub) if n == 0 => *sub,
            Self::UnorderedContainer(sub) if n == 0 => *sub,
            Self::Optional(sub) if n == 0 => *sub,
            _ => None,
        }
    }
}

impl Display for ContainerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let base = self.base_kind().to_rito_name();
        match self {
            Self::Map { key, value } => match (key, value) {
                (Some(key), Some(value)) => write!(f, "{base}[{key},{value}]"),
                (Some(key), None) | (None, Some(key)) => write!(f, "{base}[{key}]"),
                (None, None) => write!(f, "{base}"),
            },
            Self::Container(item) | Self::UnorderedContainer(item) | Self::Optional(item) => {
                match item {
                    Some(item) => write!(f, "{base}[{item}]"),
                    None => write!(f, "{base}"),
                }
            }
        }
    }
}

impl ContainerType {
    pub fn from_parts(base: PropertyKind, subtypes: [Option<PropertyKind>; 2]) -> Option<Self> {
        Some(match base {
            PropertyKind::Container => {
                Self::Container(subtypes[0].and_then(|k| ScalarType::try_from_kind(k).ok()))
            }
            PropertyKind::UnorderedContainer => Self::UnorderedContainer(
                subtypes[0].and_then(|k| ScalarType::try_from_kind(k).ok()),
            ),
            PropertyKind::Optional => {
                Self::Optional(subtypes[0].and_then(|k| ScalarType::try_from_kind(k).ok()))
            }
            PropertyKind::Map => Self::Map {
                key: subtypes[0].and_then(|k| ScalarType::try_from_kind(k).ok()),
                value: subtypes[1].and_then(|k| ScalarType::try_from_kind(k).ok()),
            },
            _ => return None,
        })
    }
    pub const fn try_from_parts(
        base: PropertyKind,
        subtypes: (PropertyKind, Option<PropertyKind>),
    ) -> Result<Self, BadContainerType> {
        use BadContainerType as E;
        match base {
            PropertyKind::Container => {
                if subtypes.1.is_some() {
                    return Err(E::BadSubtypeCount {
                        base,
                        got: 2,
                        expected: 1,
                    });
                }
                match ScalarType::try_from_kind(subtypes.0) {
                    Ok(item) => Ok(Self::Container(Some(item))),
                    Err(e) => Err(E::NonScalarSubtype { base, got: e.0 }),
                }
            }
            PropertyKind::UnorderedContainer => {
                if subtypes.1.is_some() {
                    return Err(E::BadSubtypeCount {
                        base,
                        got: 2,
                        expected: 1,
                    });
                }
                match ScalarType::try_from_kind(subtypes.0) {
                    Ok(item) => Ok(Self::UnorderedContainer(Some(item))),
                    Err(e) => Err(E::NonScalarSubtype { base, got: e.0 }),
                }
            }
            PropertyKind::Optional => {
                if subtypes.1.is_some() {
                    return Err(E::BadSubtypeCount {
                        base,
                        got: 2,
                        expected: 1,
                    });
                }
                match ScalarType::try_from_kind(subtypes.0) {
                    Ok(item) => Ok(Self::Optional(Some(item))),
                    Err(e) => Err(E::NonScalarSubtype { base, got: e.0 }),
                }
            }
            PropertyKind::Map => {
                if !subtypes.0.is_valid_map_key() {
                    return Err(E::BadMapKey { got: subtypes.0 });
                }

                let key = match ScalarType::try_from_kind(subtypes.0) {
                    Ok(key) => key,
                    Err(e) => return Err(E::NonScalarSubtype { base, got: e.0 }),
                };

                let value = match subtypes.1 {
                    Some(value) => match ScalarType::try_from_kind(value) {
                        Ok(value) => value,
                        Err(e) => return Err(E::NonScalarSubtype { base, got: e.0 }),
                    },
                    None => {
                        return Err(E::BadSubtypeCount {
                            base,
                            got: 1,
                            expected: 2,
                        })
                    }
                };
                Ok(Self::Map {
                    key: Some(key),
                    value: Some(value),
                })
            }
            got => Err(E::NonContainerBase { got }),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum BadContainerType {
    #[error("{got:?} is not a container type")]
    NonContainerBase { got: PropertyKind },

    #[error("{base:?} expects {expected} subtypes, got {got}")]
    BadSubtypeCount {
        base: PropertyKind,
        got: u8,
        expected: u8,
    },
    #[error("cannot hold {got:?} in {base:?} - containers can only hold scalars.")]
    NonScalarSubtype {
        base: PropertyKind,
        got: PropertyKind,
    },
    #[error("{got:?} is not a valid map key")]
    BadMapKey { got: PropertyKind },
}

impl From<ContainerType> for PropertyKind {
    fn from(value: ContainerType) -> Self {
        value.base_kind()
    }
}

impl ContainerType {
    pub fn equivalent_to(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Map {
                    key: l_key,
                    value: l_value,
                },
                Self::Map {
                    key: r_key,
                    value: r_value,
                },
            ) => {
                l_key.zip(*r_key).is_none_or(|(a, b)| a == b)
                    && l_value.zip(*r_value).is_none_or(|(a, b)| a == b)
            }
            (Self::Container(l0), Self::Container(r0)) => l0.zip(*r0).is_none_or(|(a, b)| a == b),
            (Self::UnorderedContainer(l0), Self::UnorderedContainer(r0)) => {
                l0.zip(*r0).is_none_or(|(a, b)| a == b)
            }
            (Self::Optional(l0), Self::Optional(r0)) => l0.zip(*r0).is_none_or(|(a, b)| a == b),
            _ => false,
        }
    }
}
