use indexmap::IndexMap;

use crate::{BinProperty, PropertyValueEnum};

use super::BinObject;

/// A builder for constructing [`Object`] instances.
///
/// # Examples
///
/// ```
/// use ltk_meta::tree::Object;
/// use ltk_meta::value;
///
/// let obj = Object::builder(0x12345678, 0xABCDEF00)
///     .property(0x1111, value::I32(42))
///     .property(0x2222, value::String("hello".into()))
///     .property(0x3333, value::Bool(true))
///     .build();
///
/// assert_eq!(obj.properties.len(), 3);
/// ```
#[derive(Debug, Clone)]
pub struct Builder {
    path_hash: u32,
    class_hash: u32,
    properties: IndexMap<u32, BinProperty>,
}

impl Builder {
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

    /// Add a [`BinProperty`]
    pub fn bin_property(mut self, prop: BinProperty) -> Self {
        self.properties.insert(prop.name_hash, prop);
        self
    }

    /// Adds a property with the given name hash and value.
    ///
    /// This is a convenience method that accepts any type that can be converted
    /// into a [`PropertyValueEnum`].
    pub fn property(mut self, name_hash: u32, value: impl Into<PropertyValueEnum>) -> Self {
        self.properties.insert(
            name_hash,
            BinProperty {
                name_hash,
                value: value.into(),
            },
        );
        self
    }

    /// Adds multiple properties from [`BinProperty`] instances.
    pub fn bin_properties(mut self, props: impl IntoIterator<Item = BinProperty>) -> Self {
        for prop in props {
            self.properties.insert(prop.name_hash, prop);
        }
        self
    }

    /// Builds the final [`Object`].
    pub fn build(self) -> BinObject {
        BinObject {
            path_hash: self.path_hash,
            class_hash: self.class_hash,
            properties: self.properties,
        }
    }
}
