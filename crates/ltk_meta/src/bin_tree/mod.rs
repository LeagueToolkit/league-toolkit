//! # BinTree
//!
//! This module provides types for reading and writing League of Legends
//! property bin files (`.bin`).
//!
//! Property bins are hierarchical data structures used throughout League's
//! game data. They contain objects with typed properties that can reference
//! other objects and external files.
//!
//! ## Quick Start
//!
//! ### Reading a bin file
//!
//! ```no_run
//! use std::fs::File;
//! use ltk_meta::BinTree;
//!
//! let mut file = File::open("data.bin")?;
//! let tree = BinTree::from_reader(&mut file)?;
//!
//! for (path_hash, object) in &tree.objects {
//!     println!("Object {:08x} has {} properties", path_hash, object.properties.len());
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ### Creating a bin file programmatically
//!
//! ```
//! use ltk_meta::{BinTree, BinTreeObject};
//! use ltk_meta::value::*;
//!
//! // Using the builder pattern
//! let tree = BinTree::builder()
//!     .dependency("common.bin")
//!     .object(
//!         BinTreeObject::builder(0x12345678, 0xABCDEF00)
//!             .property(0x1111, I32Value(42))
//!             .property(0x2222, StringValue("hello".into()))
//!             .build()
//!     )
//!     .build();
//!
//! // Or using the simple constructor
//! let tree = BinTree::new(
//!     [BinTreeObject::new(0x1234, 0x5678)],
//!     ["dependency.bin"],
//! );
//! ```
//!
//! ### Modifying a bin file
//!
//! ```no_run
//! use std::fs::File;
//! use std::io::Cursor;
//! use ltk_meta::{BinTree, BinTreeObject};
//!
//! let mut file = File::open("data.bin")?;
//! let mut tree = BinTree::from_reader(&mut file)?;
//!
//! // Add a new object
//! tree.add_object(BinTreeObject::new(0x11112222, 0x33334444));
//!
//! // Remove an object
//! tree.remove_object(0x55556666);
//!
//! // Write back
//! let mut output = Cursor::new(Vec::new());
//! tree.to_writer(&mut output)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use indexmap::IndexMap;

mod object;
pub use object::*;

mod read;
mod write;

#[cfg(test)]
mod tests;

/// The top level tree of a bin file.
///
/// A `BinTree` represents the complete contents of a League of Legends
/// property bin file. It contains a collection of objects, each identified
/// by a path hash, along with optional dependencies on other bin files.
///
/// # Construction
///
/// Use [`BinTree::new`] for simple cases or [`BinTree::builder`] for more control:
///
/// ```
/// use ltk_meta::{BinTree, BinTreeObject};
///
/// // Simple construction
/// let tree = BinTree::new([], std::iter::empty::<&str>());
///
/// // Builder pattern
/// let tree = BinTree::builder()
///     .dependency("base.bin")
///     .build();
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct BinTree {
    /// Whether this is an override/patch bin file.
    pub is_override: bool,

    /// The bin file version. When reading, this reflects the source file version.
    /// When writing, version 3 is always used regardless of this value.
    pub version: u32,

    /// The objects in this bin tree, keyed by their path hash.
    pub objects: IndexMap<u32, BinTreeObject>,

    /// List of other property bins this file depends on.
    ///
    /// Property bins can depend on other property bins in a similar fashion
    /// to importing code libraries.
    pub dependencies: Vec<String>,

    /// Data overrides (currently not fully implemented).
    data_overrides: Vec<()>,
}

impl Default for BinTree {
    fn default() -> Self {
        Self {
            version: 3,
            is_override: false,
            objects: IndexMap::new(),
            dependencies: Vec::new(),
            data_overrides: Vec::new(),
        }
    }
}

impl BinTree {
    /// Creates a new `BinTree` with the given objects and dependencies.
    ///
    /// The version is set to 3 and `is_override` is set to false.
    ///
    /// # Examples
    ///
    /// ```
    /// use ltk_meta::{BinTree, BinTreeObject};
    ///
    /// let tree = BinTree::new(
    ///     [BinTreeObject::new(0x1234, 0x5678)],
    ///     ["dependency.bin"],
    /// );
    /// ```
    pub fn new(
        objects: impl IntoIterator<Item = BinTreeObject>,
        dependencies: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            version: 3,
            is_override: false,
            objects: objects
                .into_iter()
                .map(|o: BinTreeObject| (o.path_hash, o))
                .collect(),
            dependencies: dependencies.into_iter().map(Into::into).collect(),
            data_overrides: Vec::new(),
        }
    }

    /// Creates a new builder for constructing a `BinTree`.
    ///
    /// # Examples
    ///
    /// ```
    /// use ltk_meta::{BinTree, BinTreeObject};
    ///
    /// let tree = BinTree::builder()
    ///     .dependency("common.bin")
    ///     .object(BinTreeObject::new(0x1234, 0x5678))
    ///     .build();
    /// ```
    pub fn builder() -> BinTreeBuilder {
        BinTreeBuilder::new()
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
    pub fn get_object(&self, path_hash: u32) -> Option<&BinTreeObject> {
        self.objects.get(&path_hash)
    }

    /// Returns a mutable reference to the object with the given path hash, if it exists.
    #[inline]
    pub fn get_object_mut(&mut self, path_hash: u32) -> Option<&mut BinTreeObject> {
        self.objects.get_mut(&path_hash)
    }

    /// Returns `true` if the tree contains an object with the given path hash.
    #[inline]
    pub fn contains_object(&self, path_hash: u32) -> bool {
        self.objects.contains_key(&path_hash)
    }

    /// Adds an object to the tree.
    ///
    /// If an object with the same path hash already exists, it is replaced
    /// and the old object is returned.
    pub fn add_object(&mut self, object: BinTreeObject) -> Option<BinTreeObject> {
        self.objects.insert(object.path_hash, object)
    }

    /// Removes and returns the object with the given path hash, if it exists.
    pub fn remove_object(&mut self, path_hash: u32) -> Option<BinTreeObject> {
        self.objects.shift_remove(&path_hash)
    }

    /// Adds a dependency to the tree.
    pub fn add_dependency(&mut self, dependency: impl Into<String>) {
        self.dependencies.push(dependency.into());
    }

    /// Returns an iterator over the objects in the tree.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&u32, &BinTreeObject)> {
        self.objects.iter()
    }

    /// Returns a mutable iterator over the objects in the tree.
    #[inline]
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&u32, &mut BinTreeObject)> {
        self.objects.iter_mut()
    }
}

impl<'a> IntoIterator for &'a BinTree {
    type Item = (&'a u32, &'a BinTreeObject);
    type IntoIter = indexmap::map::Iter<'a, u32, BinTreeObject>;

    fn into_iter(self) -> Self::IntoIter {
        self.objects.iter()
    }
}

impl<'a> IntoIterator for &'a mut BinTree {
    type Item = (&'a u32, &'a mut BinTreeObject);
    type IntoIter = indexmap::map::IterMut<'a, u32, BinTreeObject>;

    fn into_iter(self) -> Self::IntoIter {
        self.objects.iter_mut()
    }
}

impl IntoIterator for BinTree {
    type Item = (u32, BinTreeObject);
    type IntoIter = indexmap::map::IntoIter<u32, BinTreeObject>;

    fn into_iter(self) -> Self::IntoIter {
        self.objects.into_iter()
    }
}

/// A builder for constructing [`BinTree`] instances.
///
/// # Examples
///
/// ```
/// use ltk_meta::{BinTree, BinTreeObject, BinTreeBuilder};
///
/// let tree = BinTreeBuilder::new()
///     .is_override(false)
///     .dependency("base.bin")
///     .dependencies(["extra1.bin", "extra2.bin"])
///     .object(BinTreeObject::new(0x1234, 0x5678))
///     .build();
/// ```
#[derive(Debug, Default, Clone)]
pub struct BinTreeBuilder {
    is_override: bool,
    objects: Vec<BinTreeObject>,
    dependencies: Vec<String>,
}

impl BinTreeBuilder {
    /// Creates a new `BinTreeBuilder` with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether this is an override bin file.
    ///
    /// Default is `false`.
    pub fn is_override(mut self, is_override: bool) -> Self {
        self.is_override = is_override;
        self
    }

    /// Adds a single dependency.
    pub fn dependency(mut self, dep: impl Into<String>) -> Self {
        self.dependencies.push(dep.into());
        self
    }

    /// Adds multiple dependencies.
    pub fn dependencies(mut self, deps: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.dependencies.extend(deps.into_iter().map(Into::into));
        self
    }

    /// Adds a single object.
    pub fn object(mut self, obj: BinTreeObject) -> Self {
        self.objects.push(obj);
        self
    }

    /// Adds multiple objects.
    pub fn objects(mut self, objs: impl IntoIterator<Item = BinTreeObject>) -> Self {
        self.objects.extend(objs);
        self
    }

    /// Builds the [`BinTree`].
    ///
    /// The resulting tree will have version 3, which is always used when writing.
    pub fn build(self) -> BinTree {
        BinTree {
            version: 3,
            is_override: self.is_override,
            objects: self.objects.into_iter().map(|o| (o.path_hash, o)).collect(),
            dependencies: self.dependencies,
            data_overrides: Vec::new(),
        }
    }
}
