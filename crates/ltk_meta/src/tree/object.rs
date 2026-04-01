//! Bin object types and builders.

mod builder;
pub use builder::Builder;

use std::io;

use indexmap::IndexMap;
use ltk_io_ext::{measure, window_at};

use crate::{
    property::NoMeta,
    traits::{ReaderExt, WriterExt},
};

use super::super::{Error, PropertyValueEnum};
use byteorder::{ReadBytesExt, WriteBytesExt, LE};

/// A node/object in the bin tree.
///
/// Each object has a path hash (unique identifier), a class hash (type identifier),
/// and a collection of properties.
///
/// # Construction
///
/// Use [`BinObject::new`] for simple cases or [`BinObject::builder`] for
/// adding properties inline:
///
/// ```
/// use ltk_meta::BinObject;
/// use ltk_meta::property::{values, NoMeta};
///
/// // Simple construction
/// let obj = BinObject::<NoMeta>::new(0x1234, 0x5678);
///
/// // Builder pattern with properties
/// let obj = BinObject::<NoMeta>::builder(0x1234, 0x5678)
///     .property(0xAAAA, values::I32::new(42))
///     .property(0xBBBB, values::String::from("hello"))
///     .build();
/// ```
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(bound = "for <'dee> M: serde::Serialize + serde::Deserialize<'dee>")
)]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BinObject<M = NoMeta> {
    /// The unique path hash identifying this object.
    pub path_hash: u32,

    /// The class hash identifying the type/schema of this object.
    pub class_hash: u32,

    /// The properties of this object, keyed by their name hash.
    pub properties: IndexMap<u32, PropertyValueEnum<M>>,
}

impl<M> BinObject<M> {
    /// Creates a new `BinObject` with the given path and class hashes.
    ///
    /// The object starts with no properties.
    ///
    /// # Examples
    ///
    /// ```
    /// use ltk_meta::{property::NoMeta, BinObject};
    ///
    /// let obj = BinObject::<NoMeta>::new(0x12345678, 0xABCDEF00);
    /// assert!(obj.properties.is_empty());
    /// ```
    pub fn new(path_hash: u32, class_hash: u32) -> Self {
        Self {
            path_hash,
            class_hash,
            properties: IndexMap::default(),
        }
    }

    /// Creates a new builder for constructing a `BinObject`.
    ///
    /// # Examples
    ///
    /// ```
    /// use ltk_meta::BinObject;
    /// use ltk_meta::property::{values, NoMeta};
    ///
    /// let obj = BinObject::<NoMeta>::builder(0x12345678, 0xABCDEF00)
    ///     .property(0x1111, values::I32::new(42))
    ///     .property(0x2222, values::String::from("hello"))
    ///     .property(0x3333, values::Bool::new(true))
    ///     .build();
    ///
    /// assert_eq!(obj.properties.len(), 3);
    /// ```
    pub fn builder(path_hash: u32, class_hash: u32) -> builder::Builder {
        builder::Builder::new(path_hash, class_hash)
    }

    /// Reads a BinObject from a reader.
    ///
    /// # Arguments
    ///
    /// * `reader` - A reader that implements [`io::Read`] and [`io::Seek`].
    /// * `class_hash` - The hash of the class of the object.
    /// * `legacy` - Whether to read in legacy format.
    pub fn from_reader<R: io::Read + io::Seek + ?Sized>(
        reader: &mut R,
        class_hash: u32,
        legacy: bool,
    ) -> Result<Self, Error>
    where
        M: Default,
    {
        let size = reader.read_u32::<LE>()?;
        let (real_size, value) = measure(reader, |reader| {
            let path_hash = reader.read_u32::<LE>()?;

            let prop_count = reader.read_u16::<LE>()? as usize;
            let mut properties = IndexMap::with_capacity(prop_count);
            for _ in 0..prop_count {
                let (name_hash, value) = reader.read_property::<M>(legacy)?;
                properties.insert(name_hash, value);
            }

            Ok::<_, Error>(Self {
                path_hash,
                class_hash,
                properties,
            })
        })?;

        if size as u64 != real_size {
            return Err(Error::InvalidSize(size as _, real_size));
        }
        Ok(value)
    }

    /// Writes this object to a writer.
    ///
    /// # Arguments
    ///
    /// * `writer` - A writer that implements io::Write and io::Seek.
    pub fn to_writer<W: io::Write + io::Seek + ?Sized>(&self, writer: &mut W) -> io::Result<()>
    where
        M: Clone,
    {
        let size_pos = writer.stream_position()?;
        writer.write_u32::<LE>(0)?;

        let (size, _) = measure(writer, |writer| {
            writer.write_u32::<LE>(self.path_hash)?;
            writer.write_u16::<LE>(self.properties.len() as _)?;
            for (name_hash, value) in self.properties.iter() {
                writer.write_property(*name_hash, value)?;
            }
            Ok::<_, io::Error>(())
        })?;

        window_at(writer, size_pos, |writer| writer.write_u32::<LE>(size as _))?;
        Ok(())
    }

    /// Returns the number of properties in this object.
    #[must_use]
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.properties.len()
    }

    /// Returns `true` if this object has no properties.
    #[must_use]
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.properties.is_empty()
    }

    /// Returns a reference to the property with the given name hash, if it exists.
    #[must_use]
    #[inline(always)]
    pub fn get_property(&self, name_hash: u32) -> Option<&PropertyValueEnum<M>> {
        self.properties.get(&name_hash)
    }

    /// Returns a mutable reference to the property with the given name hash, if it exists.
    #[must_use]
    #[inline(always)]
    pub fn get_property_mut(&mut self, name_hash: u32) -> Option<&mut PropertyValueEnum<M>> {
        self.properties.get_mut(&name_hash)
    }

    /// Returns `true` if this object has a property with the given name hash.
    #[must_use]
    #[inline(always)]
    pub fn contains_property(&self, name_hash: u32) -> bool {
        self.properties.contains_key(&name_hash)
    }

    /// Adds or replaces a property.
    ///
    /// If a property with the same name hash already exists, it is replaced
    /// and the old property is returned.
    #[inline(always)]
    pub fn insert(
        &mut self,
        name_hash: u32,
        value: impl Into<PropertyValueEnum<M>>,
    ) -> Option<PropertyValueEnum<M>> {
        self.properties.insert(name_hash, value.into())
    }

    /// Removes and returns the property with the given name hash, if it exists.
    pub fn remove_property(&mut self, name_hash: u32) -> Option<PropertyValueEnum<M>> {
        self.properties.shift_remove(&name_hash)
    }

    /// Returns an iterator over the properties in this object.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&u32, &PropertyValueEnum<M>)> {
        self.properties.iter()
    }

    /// Returns a mutable iterator over the properties in this object.
    #[inline]
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&u32, &mut PropertyValueEnum<M>)> {
        self.properties.iter_mut()
    }
}

impl<'a, M> IntoIterator for &'a BinObject<M> {
    type Item = (&'a u32, &'a PropertyValueEnum<M>);
    type IntoIter = indexmap::map::Iter<'a, u32, PropertyValueEnum<M>>;

    fn into_iter(self) -> Self::IntoIter {
        self.properties.iter()
    }
}

impl<'a, M> IntoIterator for &'a mut BinObject<M> {
    type Item = (&'a u32, &'a mut PropertyValueEnum<M>);
    type IntoIter = indexmap::map::IterMut<'a, u32, PropertyValueEnum<M>>;

    fn into_iter(self) -> Self::IntoIter {
        self.properties.iter_mut()
    }
}

impl<M> IntoIterator for BinObject<M> {
    type Item = (u32, PropertyValueEnum<M>);
    type IntoIter = indexmap::map::IntoIter<u32, PropertyValueEnum<M>>;

    fn into_iter(self) -> Self::IntoIter {
        self.properties.into_iter()
    }
}
