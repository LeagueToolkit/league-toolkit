use indexmap::IndexMap;
use ltk_hash::BinHash;

use crate::{property::NoMeta, BinObject, PropertyValueEnum};

/// A builder for constructing [`BinObject`] instances.
///
/// See: [`BinObject::builder`]
#[derive(Debug, Clone)]
pub struct Builder<M = NoMeta> {
    path_hash: BinHash,
    class_hash: BinHash,
    properties: IndexMap<BinHash, PropertyValueEnum<M>>,
}

impl<M> Builder<M> {
    /// See: [`BinObject::builder`]
    pub fn new(path_hash: impl Into<BinHash>, class_hash: impl Into<BinHash>) -> Self {
        Self {
            path_hash: path_hash.into(),
            class_hash: class_hash.into(),
            properties: IndexMap::new(),
        }
    }

    pub fn path_hash(mut self, path_hash: impl Into<BinHash>) -> Self {
        self.path_hash = path_hash.into();
        self
    }

    pub fn class_hash(mut self, class_hash: impl Into<BinHash>) -> Self {
        self.class_hash = class_hash.into();
        self
    }

    /// Adds a property with the given name hash and value.
    pub fn property(
        mut self,
        name_hash: impl Into<BinHash>,
        value: impl Into<PropertyValueEnum<M>>,
    ) -> Self {
        self.properties.insert(name_hash.into(), value.into());
        self
    }

    /// Adds multiple properties from an iterator of name hashes & [`PropertyValueEnum`]s.
    pub fn properties(
        mut self,
        props: impl IntoIterator<Item = (BinHash, PropertyValueEnum<M>)>,
    ) -> Self {
        self.properties.extend(props);
        self
    }

    /// Builds the final [`BinObject`].
    pub fn build(self) -> BinObject<M> {
        BinObject {
            path_hash: self.path_hash,
            class_hash: self.class_hash,
            properties: self.properties,
        }
    }
}
