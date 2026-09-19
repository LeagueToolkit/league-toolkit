mod builder;
pub use builder::Builder;

mod object;
use ltk_hash::BinHash;
pub use object::{BinObject, Builder as ObjectBuilder};

pub(crate) mod read;
pub(crate) mod write;

#[cfg(test)]
mod tests;

use indexmap::IndexMap;

/// The complete contents of a League of Legends property bin file.
///
/// It contains a collection of objects, each identified
/// by a path hash, along with a list of any dependencies on other bin files.
///
/// # Construction
///
/// Use [`Bin::new`] for simple cases or [`Bin::builder`] for more control:
///
/// ```
/// use ltk_meta::{Bin, BinObject};
///
/// // Simple construction
/// let tree = Bin::new([], std::iter::empty::<&str>());
///
/// // Builder pattern
/// let tree = Bin::builder()
///     .dependency("base.bin")
///     .build();
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Bin {
    /// The bin file version. When reading, this reflects the source file version.
    /// When writing, version 3 is always used regardless of this value.
    pub version: u32,

    /// The objects in this bin tree, keyed by their path hash.
    pub objects: IndexMap<BinHash, BinObject>,

    /// List of other property bins this file depends on.
    ///
    /// Property bins can depend on other property bins in a similar fashion
    /// to importing code libraries.
    pub dependencies: Vec<String>,
}

impl Default for Bin {
    fn default() -> Self {
        Self {
            version: 3,
            objects: IndexMap::new(),
            dependencies: Vec::new(),
        }
    }
}

impl Bin {
    /// Creates a new `Bin` with the given objects and dependencies.
    ///
    /// The version is set to 3.
    ///
    /// # Examples
    ///
    /// ```
    /// use ltk_meta::{Bin, BinObject};
    ///
    /// let tree = Bin::new(
    ///     [BinObject::new(0x1234, 0x5678)],
    ///     ["dependency.bin"],
    /// );
    /// ```
    pub fn new(
        objects: impl IntoIterator<Item = BinObject>,
        dependencies: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            version: 3,
            objects: objects
                .into_iter()
                .map(|o: BinObject| (o.path_hash, o))
                .collect(),
            dependencies: dependencies.into_iter().map(Into::into).collect(),
        }
    }

    /// Creates a new builder for constructing a `Bin`.
    ///
    /// # Examples
    ///
    /// ```
    /// use ltk_meta::{Bin, BinObject};
    ///
    /// let tree = Bin::builder()
    ///     .dependency("common.bin")
    ///     .object(BinObject::new(0x1234, 0x5678))
    ///     .build();
    /// ```
    pub fn builder() -> builder::Builder {
        builder::Builder::new()
    }

    /// Returns the number of objects in the tree.
    #[inline]
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Returns `true` if the tree contains no objects.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// Returns a reference to the object with the given path hash, if it exists.
    #[inline]
    pub fn get_object(&self, path_hash: BinHash) -> Option<&BinObject> {
        self.objects.get(&path_hash)
    }

    /// Returns a mutable reference to the object with the given path hash, if it exists.
    #[inline]
    pub fn get_object_mut(&mut self, path_hash: BinHash) -> Option<&mut BinObject> {
        self.objects.get_mut(&path_hash)
    }

    /// Returns `true` if the tree contains an object with the given path hash.
    #[inline]
    pub fn contains_object(&self, path_hash: BinHash) -> bool {
        self.objects.contains_key(&path_hash)
    }

    /// Adds an object to the tree.
    ///
    /// If an object with the same path hash already exists, it is replaced
    /// and the old object is returned.
    pub fn add_object(&mut self, object: BinObject) -> Option<BinObject> {
        self.objects.insert(object.path_hash, object)
    }

    /// Removes and returns the object with the given path hash, if it exists.
    pub fn remove_object(&mut self, path_hash: impl Into<BinHash>) -> Option<BinObject> {
        self.objects.shift_remove(&path_hash.into())
    }

    /// Adds a dependency to the tree.
    pub fn add_dependency(&mut self, dependency: impl Into<String>) {
        self.dependencies.push(dependency.into());
    }

    /// Returns an iterator over the objects in the tree.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&BinHash, &BinObject)> {
        self.objects.iter()
    }

    /// Returns a mutable iterator over the objects in the tree.
    #[inline]
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&BinHash, &mut BinObject)> {
        self.objects.iter_mut()
    }
}

impl<'a> IntoIterator for &'a Bin {
    type Item = (&'a BinHash, &'a BinObject);
    type IntoIter = indexmap::map::Iter<'a, BinHash, BinObject>;

    fn into_iter(self) -> Self::IntoIter {
        self.objects.iter()
    }
}

impl<'a> IntoIterator for &'a mut Bin {
    type Item = (&'a BinHash, &'a mut BinObject);
    type IntoIter = indexmap::map::IterMut<'a, BinHash, BinObject>;

    fn into_iter(self) -> Self::IntoIter {
        self.objects.iter_mut()
    }
}

impl IntoIterator for Bin {
    type Item = (BinHash, BinObject);
    type IntoIter = indexmap::map::IntoIter<BinHash, BinObject>;

    fn into_iter(self) -> Self::IntoIter {
        self.objects.into_iter()
    }
}
