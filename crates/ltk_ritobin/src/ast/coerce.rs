use ltk_hash::{BinHash, Hash as _, WadHash};
use ltk_meta::{property::values, PropertyKind, PropertyValueEnum};
use std::fmt::Debug;

use crate::RitoType;

pub trait CanCoerce {
    fn can_coerce(self, from: Self) -> bool;
}

pub trait CoerceFrom {
    fn coerce_from<M: Debug + Default>(
        self,
        value: PropertyValueEnum<M>,
    ) -> Option<PropertyValueEnum<M>>;
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
impl CoerceFrom for PropertyKind {
    fn coerce_from<M: Debug + Default>(
        self,
        value: PropertyValueEnum<M>,
    ) -> Option<PropertyValueEnum<M>> {
        let to = self;
        match to {
            to if to == value.kind() => Some(value),

            PropertyKind::Optional => Some(values::Optional::try_from(value).ok()?.into()),

            PropertyKind::Hash => match value {
                PropertyValueEnum::String(str) => {
                    Some(values::Hash::new_with_meta(BinHash::hash_str(&str), str.meta).into())
                }
                _ => None,
            },
            PropertyKind::ObjectLink => match value {
                PropertyValueEnum::Hash(hash) => {
                    Some(values::ObjectLink::new_with_meta(*hash, hash.meta).into())
                }
                PropertyValueEnum::String(str) => Some(
                    values::ObjectLink::new_with_meta(BinHash::hash_str(&str), str.meta).into(),
                ),
                _ => None,
            },
            PropertyKind::WadChunkLink => match value {
                PropertyValueEnum::Hash(hash) => Some(
                    values::WadChunkLink::new_with_meta(WadHash((**hash).into()), hash.meta).into(),
                ),
                PropertyValueEnum::String(str) => Some(
                    values::WadChunkLink::new_with_meta(WadHash::hash_str(str.as_str()), str.meta)
                        .into(),
                ),
                _ => None,
            },
            PropertyKind::BitBool => match value {
                PropertyValueEnum::Bool(bool) => {
                    Some(values::BitBool::new_with_meta(*bool, bool.meta).into())
                }
                _ => None,
            },
            PropertyKind::Bool => match value {
                PropertyValueEnum::BitBool(bool) => {
                    Some(values::Bool::new_with_meta(*bool, bool.meta).into())
                }
                _ => None,
            },
            _ => None,
        }
    }
}
