use ltk_hash::{BinHash, Hash as _, WadHash};
use ltk_meta::PropertyKind;

use crate::{
    ast::{node::Value, Ptr},
    RitoType,
};

use super::hash::{HashedLiteral, Originally};

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
impl CanCoerce for RitoType {
    fn can_coerce(self, from: Self) -> bool {
        if !self.base.can_coerce(from.base) {
            return false;
        }
        for i in 0..1 {
            if (self.subtypes[i].zip(from.subtypes[i]))
                .is_some_and(|(to, from)| !to.can_coerce(from))
            {
                return false;
            }
        }
        true
    }
}

impl Value {
    pub fn coerce_to(self, to: PropertyKind) -> Option<Self> {
        Some(match to {
            to if to == self.kind() => self,

            PropertyKind::Optional => Self::Optional {
                item_kind: self.kind(),
                span: self.span(),
                value: Some(Ptr::new(self)),
            },

            PropertyKind::Hash => match self {
                Self::String(str) => Self::Hash(HashedLiteral::new(
                    str.span,
                    Originally::String,
                    BinHash::hash_str(&str),
                )),
                _ => return None,
            },
            PropertyKind::ObjectLink => match self {
                Self::Hash(hash) => Self::ObjectLink(hash),
                Self::String(str) => Self::ObjectLink(HashedLiteral::new(
                    str.span,
                    Originally::String,
                    BinHash::hash_str(&str),
                )),
                _ => return None,
            },
            PropertyKind::WadChunkLink => match self {
                Self::Hash(hash) => {
                    Self::WadChunkLink(hash.with_value(WadHash((*hash.value).into())))
                }
                Self::String(str) => Self::WadChunkLink(HashedLiteral::new(
                    str.span,
                    Originally::String,
                    WadHash::hash_str(str.as_str()),
                )),
                _ => return None,
            },
            PropertyKind::BitBool => match self {
                Self::Bool(bool) => Self::BitBool(bool),
                _ => return None,
            },
            PropertyKind::Bool => match self {
                Self::BitBool(bool) => Self::Bool(bool),
                _ => return None,
            },
            _ => return None,
        })
    }
}
