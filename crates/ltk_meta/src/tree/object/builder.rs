use indexmap::IndexMap;

use crate::{property::NoMeta, BinObject, PropertyValueEnum};

/// A builder for constructing [`BinObject`] instances.
///
/// See: [`BinObject::builder`]
#[derive(Debug, Clone)]
pub struct Builder<M = NoMeta> {
    path_hash: u32,
    class_hash: u32,
    properties: IndexMap<u32, PropertyValueEnum<M>>,
}

impl<M> Builder<M> {
    /// See: [`BinObject::builder`]
    pub fn new(path_hash: u32, class_hash: u32) -> Self {
        Self {
            path_hash,
            class_hash,
            properties: IndexMap::new(),
        }
    }

    pub fn path_hash(mut self, path_hash: u32) -> Self {
        self.path_hash = path_hash;
        self
    }

    pub fn class_hash(mut self, class_hash: u32) -> Self {
        self.class_hash = class_hash;
        self
    }

    /// Adds a property with the given name hash and value.
    pub fn property(mut self, name_hash: u32, value: impl Into<PropertyValueEnum<M>>) -> Self {
        self.properties.insert(name_hash, value.into());
        self
    }

    /// Adds multiple properties from an iterator of name hashes & [`PropertyValueEnum`]s.
    pub fn properties(
        mut self,
        props: impl IntoIterator<Item = (u32, PropertyValueEnum<M>)>,
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
