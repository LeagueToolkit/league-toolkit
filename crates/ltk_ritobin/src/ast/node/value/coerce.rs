use ltk_hash::{BinHash, Hash as _, HashValue, WadHash};
use ltk_meta::PropertyKind;

use crate::{
    ast::{node::Value, Ptr},
    ContainerType, HashType, RitoType, ScalarType,
};

use crate::ast::hash::{HashedLiteral, Originally};

pub trait CanCoerce {
    fn can_coerce(self, from: Self) -> bool;
}

impl CanCoerce for PropertyKind {
    fn can_coerce(self, from: Self) -> bool {
        let to = self;
        if to == from {
            return true;
        }
        use PropertyKind as K;
        match (to, from) {
            (K::Optional, from) if !from.is_container() => true,
            (K::Hash, K::String)
            | (K::WadChunkLink | K::ObjectLink, K::Hash | K::String)
            | (K::BitBool | K::Bool, K::Bool | K::BitBool) => true,
            _ => false,
        }
    }
}
impl CanCoerce for ScalarType {
    fn can_coerce(self, from: Self) -> bool {
        self.kind().can_coerce(from.kind())
    }
}
impl CanCoerce for RitoType {
    fn can_coerce(self, from: Self) -> bool {
        if !self.base().can_coerce(from.base()) {
            return false;
        }
        for i in 0..1 {
            if (self.subtype(i).zip(from.subtype(i))).is_some_and(|(to, from)| !to.can_coerce(from))
            {
                return false;
            }
        }
        true
    }
}

impl Value {
    pub fn coerce_to(self, to: impl Into<RitoType>) -> Self {
        match self.try_coerce_to(to) {
            Ok(v) => v,
            Err(v) => v,
        }
    }

    /// Converts the value to the base kind of `to`. A `hash` value takes the width of the hash
    /// type of `to`. A string in a `hash` position is hashed with the hash type of `to`.
    ///
    /// # Errors
    /// Returns the value unchanged if it does not convert.
    pub fn try_coerce_to(self, to: impl Into<RitoType>) -> Result<Self, Self> {
        let to = to.into();
        Ok(match to {
            RitoType::Scalar(ScalarType::Hash(hash_type)) => {
                return self.try_coerce_to_hash(hash_type)
            }
            to if self.rito_type().is_some_and(|k| k.equivalent_to(&to)) => self,
            // Unknown values can be coerced into any value (as an unresolved variant)
            to if self.kind().is_none() => Self::Unresolved {
                span: self.span(),
                kind: to,
            },

            RitoType::Container(ContainerType::Optional(Some(item_kind))) => Self::Optional {
                item_kind: Some(item_kind),
                span: self.span(),
                value: Some(Ptr::new(self.try_coerce_to(RitoType::from(item_kind))?)),
            },

            RitoType::Scalar(scalar) => match scalar {
                ScalarType::ObjectLink => match self {
                    Self::Hash(hash) => match hash.try_as_bin_hash() {
                        Some(hash) => Self::ObjectLink(hash),
                        None => return Err(self),
                    },
                    Self::String(str) => Self::ObjectLink(HashedLiteral::new(
                        str.span,
                        Originally::String,
                        BinHash::hash_str(&str),
                    )),
                    _ => return Err(self),
                },
                ScalarType::WadChunkLink => match self {
                    Self::Hash(hash) => {
                        Self::WadChunkLink(hash.with_value(WadHash(hash.value.as_u64())))
                    }
                    Self::String(str) => Self::WadChunkLink(HashedLiteral::new(
                        str.span,
                        Originally::String,
                        WadHash::hash_str(str.as_str()),
                    )),
                    _ => return Err(self),
                },
                ScalarType::BitBool => match self {
                    Self::Bool(bool) => Self::BitBool(bool),
                    _ => return Err(self),
                },
                ScalarType::Bool => match self {
                    Self::BitBool(bool) => Self::Bool(bool),
                    _ => return Err(self),
                },
                _ => return Err(self),
            },

            _ => return Err(self),
        })
    }

    /// Converts the value to a 4-byte hash literal of [`HashType::DEFAULT`].
    ///
    /// # Errors
    /// Returns the value if it does not convert.
    pub fn try_coerce_to_bin_hash(self) -> Result<HashedLiteral<BinHash>, Self> {
        match self.try_coerce_to(ScalarType::Hash(HashType::DEFAULT))? {
            Self::Hash(hash) => hash.try_as_bin_hash().ok_or(Self::Hash(hash)),
            other => Err(other),
        }
    }

    /// Converts the value to a `hash` value of `hash_type`.
    ///
    /// A hex literal takes the width of `hash_type`. A string is hashed with the hash function
    /// of `hash_type`.
    ///
    /// # Errors
    /// Returns the value unchanged if it is not a hash literal or a string, if the literal does
    /// not fit in the width of `hash_type`, or if it is a string and `hash_type` does not give
    /// a hash function (`hash[u32]`, `hash[u64]`).
    fn try_coerce_to_hash(self, hash_type: HashType) -> Result<Self, Self> {
        let literal = match &self {
            Self::Hash(hash) => Some(hash.value.as_u64()),
            Self::WadChunkLink(hash) if hash.was_hash() => Some(*hash.value),
            _ => None,
        };
        if let Some(literal) = literal {
            return match (HashValue::try_with_width(literal, hash_type.width), self) {
                (Some(value), Self::Hash(hash)) => Ok(Self::Hash(hash.with_value(value))),
                (Some(value), Self::WadChunkLink(hash)) => Ok(Self::Hash(hash.with_value(value))),
                (_, this) => Err(this),
            };
        }
        Ok(match self {
            Self::String(str) => match hash_type.hash_str(&str) {
                Some(value) => Self::Hash(HashedLiteral::new(str.span, Originally::String, value)),
                None => return Err(Self::String(str)),
            },
            Self::Unknown(span) => Self::Unresolved {
                span,
                kind: ScalarType::Hash(hash_type).into(),
            },
            Self::Unresolved {
                kind: RitoType::Scalar(ScalarType::Hash(_)),
                span,
            } => Self::Unresolved {
                span,
                kind: ScalarType::Hash(hash_type).into(),
            },
            this => return Err(this),
        })
    }
}
