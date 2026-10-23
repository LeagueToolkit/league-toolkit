use std::fmt::Display;

use ltk_meta::PropertyKind;

use crate::{HashType, RitobinName as _, ScalarType};

/// A ritobin container type and its subtypes.
///
/// A subtype is `None` where the type does not give it, as in a bare `list`.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum ContainerType {
    /// `map[key,value]`
    Map {
        /// The key type.
        key: Option<ScalarType>,
        /// The value type.
        value: Option<ScalarType>,
    },
    /// `list[item]`
    Container(Option<ScalarType>),
    /// `list2[item]`, the type the client names `OrderedList`.
    ///
    /// It is serialized the same as [`ContainerType::Container`]. It has no fixed size. Its
    /// [`PropertyKind`] is [`PropertyKind::UnorderedContainer`].
    OrderedList(Option<ScalarType>),
    /// `option[item]`
    Optional(Option<ScalarType>),
}

impl ContainerType {
    /// Returns the type with `hash_types` in its `hash` subtypes.
    ///
    /// The first subtype takes `hash_types[0]`. A map value takes `hash_types[1]`.
    pub fn with_hash_types(self, hash_types: [HashType; 2]) -> Self {
        match self {
            ContainerType::Map { key, value } => Self::Map {
                key: key.map(|k| k.with_hash_type(hash_types[0])),
                value: value.map(|k| k.with_hash_type(hash_types[1])),
            },
            ContainerType::Container(sub) => {
                Self::Container(sub.map(|sub| sub.with_hash_type(hash_types[0])))
            }
            ContainerType::OrderedList(sub) => {
                Self::OrderedList(sub.map(|sub| sub.with_hash_type(hash_types[0])))
            }
            ContainerType::Optional(sub) => {
                Self::Optional(sub.map(|sub| sub.with_hash_type(hash_types[0])))
            }
        }
    }
    /// Returns the type with [`HashType::DEFAULT`] in each `hash` subtype.
    pub fn with_default_hash_types(self) -> Self {
        self.with_hash_types([HashType::DEFAULT, HashType::DEFAULT])
    }

    /// Returns the [`PropertyKind`] of the container, without its subtypes.
    pub const fn base_kind(&self) -> PropertyKind {
        use PropertyKind as K;
        match self {
            ContainerType::Map { .. } => K::Map,
            ContainerType::Container(..) => K::Container,
            ContainerType::OrderedList(..) => K::UnorderedContainer,
            ContainerType::Optional(..) => K::Optional,
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
            Self::OrderedList(sub) if n == 0 => *sub,
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
            Self::Container(item) | Self::OrderedList(item) | Self::Optional(item) => match item {
                Some(item) => write!(f, "{base}[{item}]"),
                None => write!(f, "{base}"),
            },
        }
    }
}

impl ContainerType {
    /// Returns the container type of `base` with `subtypes`.
    ///
    /// Returns `None` if `base` is not a container kind. A subtype that is not a scalar kind is
    /// `None` in the result.
    pub fn from_parts(base: PropertyKind, subtypes: [Option<PropertyKind>; 2]) -> Option<Self> {
        let [first, second] =
            subtypes.map(|sub| sub.and_then(|k| ScalarType::try_from_kind(k).ok()));
        Some(match base {
            PropertyKind::Container => Self::Container(first),
            PropertyKind::UnorderedContainer => Self::OrderedList(first),
            PropertyKind::Optional => Self::Optional(first),
            PropertyKind::Map => Self::Map {
                key: first,
                value: second,
            },
            _ => return None,
        })
    }
    /// Returns the container type of `base` with `subtypes`.
    ///
    /// # Errors
    /// Returns [`BadContainerType`] if `base` is not a container kind, if the subtype count does
    /// not fit `base`, if a subtype is a container kind, or if a map key is not a valid key kind.
    pub const fn try_from_parts(
        base: PropertyKind,
        subtypes: (PropertyKind, Option<PropertyKind>),
    ) -> Result<Self, BadContainerType> {
        use BadContainerType as E;
        let item = match base {
            PropertyKind::Container | PropertyKind::UnorderedContainer | PropertyKind::Optional => {
                if subtypes.1.is_some() {
                    return Err(E::BadSubtypeCount {
                        base,
                        got: 2,
                        expected: 1,
                    });
                }
                match ScalarType::try_from_kind(subtypes.0) {
                    Ok(item) => Some(item),
                    Err(e) => return Err(E::NonScalarSubtype { base, got: e.0 }),
                }
            }
            _ => None,
        };
        match base {
            PropertyKind::Container => Ok(Self::Container(item)),
            PropertyKind::UnorderedContainer => Ok(Self::OrderedList(item)),
            PropertyKind::Optional => Ok(Self::Optional(item)),
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

/// The error for a container type that cannot be built from its parts.
#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadContainerType {
    /// The base kind is not a container kind.
    #[error("{got:?} is not a container type")]
    NonContainerBase {
        /// The base kind.
        got: PropertyKind,
    },

    /// The subtype count does not fit the base kind.
    #[error("{base:?} expects {expected} subtypes, got {got}")]
    BadSubtypeCount {
        /// The base kind.
        base: PropertyKind,
        /// The subtype count given.
        got: u8,
        /// The subtype count of `base`.
        expected: u8,
    },
    /// A subtype is a container kind. A container holds scalars only.
    #[error("cannot hold {got:?} in {base:?} - containers can only hold scalars.")]
    NonScalarSubtype {
        /// The base kind.
        base: PropertyKind,
        /// The subtype kind.
        got: PropertyKind,
    },
    /// The map key kind is not a valid key kind.
    #[error("{got:?} is not a valid map key")]
    BadMapKey {
        /// The key kind.
        got: PropertyKind,
    },
}

impl From<ContainerType> for PropertyKind {
    fn from(value: ContainerType) -> Self {
        value.base_kind()
    }
}

impl ContainerType {
    /// Returns whether `self` and `other` are the same container type.
    ///
    /// A subtype that is `None` on either side matches any subtype. Hash types compare in full.
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
            (Self::OrderedList(l0), Self::OrderedList(r0)) => {
                l0.zip(*r0).is_none_or(|(a, b)| a == b)
            }
            (Self::Optional(l0), Self::Optional(r0)) => l0.zip(*r0).is_none_or(|(a, b)| a == b),
            _ => false,
        }
    }
}
