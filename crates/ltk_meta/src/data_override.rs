//! `PTCH` bins: a patch of deletions, added objects and property patch records over a base bin.

mod read;
mod write;

use indexmap::IndexMap;
use ltk_hash::BinHash;

use crate::{
    path::PropertyPath,
    property::{Kind, NoMeta},
    BinObject, PropertyValueEnum,
};

/// The contents of a `PTCH` bin file: a patch applied over exactly one base [`Bin`](crate::Bin).
///
/// A patch bin does three things, in this order: it drops the objects named in [`deleted`], it
/// adds the whole objects in [`objects`], and it overwrites individual properties of the merged
/// table with the records in [`patches`].
///
/// The client only ever loads a `PTCH` as a patch on top of a base bin; it is never a file's
/// root data and cannot be pulled in as a dependency.
///
/// [`deleted`]: Self::deleted
/// [`objects`]: Self::objects
/// [`patches`]: Self::patches
///
/// # Examples
///
/// ```
/// use ltk_meta::{path::PropertyPath, property::{values, NoMeta}, BinOverride};
///
/// let patch_bin = BinOverride::<NoMeta>::builder()
///     .set(
///         0x4a47c414_u32,
///         PropertyPath::new("Position.Anchors.Anchor")?,
///         values::Vector2::new(glam::Vec2::new(0.0, 1.0)),
///     )
///     .build();
///
/// assert_eq!(patch_bin.patches.len(), 1);
/// # Ok::<(), ltk_meta::path::PropertyPathError>(())
/// ```
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(bound = "for <'dee> M: serde::Serialize + serde::Deserialize<'dee>")
)]
#[derive(Debug, Clone, PartialEq)]
pub struct BinOverride<M = NoMeta> {
    /// The path hashes of the objects this patch removes.
    ///
    /// The client tests every object it reads against this set, so an object is dropped whether it
    /// comes from the base bin or from a patch.
    pub deleted: Vec<BinHash>,

    /// The objects this patch adds, keyed by their path hash.
    pub objects: IndexMap<BinHash, BinObject<M>>,

    /// The property patches, in file order.
    pub patches: Vec<PropertyPatch<M>>,
}

impl Default for BinOverride {
    fn default() -> Self {
        Self::new()
    }
}

impl<M> BinOverride<M> {
    /// Creates an empty patch.
    #[must_use]
    pub fn new() -> Self {
        Self {
            deleted: Vec::new(),
            objects: IndexMap::new(),
            patches: Vec::new(),
        }
    }

    /// Creates a new builder for constructing a `BinOverride`.
    ///
    /// # Examples
    ///
    /// ```
    /// use ltk_meta::{property::NoMeta, BinObject, BinOverride};
    ///
    /// let patch_bin = BinOverride::<NoMeta>::builder()
    ///     .delete(0xdeadbeef_u32)
    ///     .object(BinObject::new(0x1234, 0x5678))
    ///     .build();
    /// ```
    #[must_use]
    pub fn builder() -> Builder<M> {
        Builder::new()
    }

    /// Returns `true` if the patch changes nothing
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.deleted.is_empty() && self.objects.is_empty() && self.patches.is_empty()
    }
}

/// One patch record: set the property at [`path`] inside object [`object_hash`] to [`value`].
///
/// The path is relative to the object, so `Position.Anchors.Anchor` walks from the object down
/// to the property it names. Resolution is the client's: a `Pointer` is dereferenced,
/// an `Embed` is descended, `[i]` indexes a container and `{k}` looks up a map entry.
///
/// [`path`]: Self::path
/// [`object_hash`]: Self::object_hash
/// [`value`]: Self::value
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(bound = "for <'dee> M: serde::Serialize + serde::Deserialize<'dee>")
)]
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyPatch<M = NoMeta> {
    /// The path hash of the object this record patches.
    pub object_hash: BinHash,

    /// The property inside that object.
    pub path: PropertyPath,

    /// The value to write.
    pub value: PropertyValueEnum<M>,
}

impl<M> PropertyPatch<M> {
    /// Creates a patch record.
    ///
    /// # Examples
    ///
    /// ```
    /// use ltk_meta::{path::PropertyPath, property::{values, NoMeta}, PropertyPatch};
    ///
    /// let patch = PropertyPatch::<NoMeta>::new(
    ///     0xa4edcb0d_u32,
    ///     PropertyPath::new("FlipX")?,
    ///     values::Bool::new(true),
    /// );
    /// assert_eq!(patch.kind(), ltk_meta::PropertyKind::Bool);
    /// # Ok::<(), ltk_meta::path::PropertyPathError>(())
    /// ```
    pub fn new(
        object_hash: impl Into<BinHash>,
        path: PropertyPath,
        value: impl Into<PropertyValueEnum<M>>,
    ) -> Self {
        Self {
            object_hash: object_hash.into(),
            path,
            value: value.into(),
        }
    }

    /// The kind tag written to the file, which is always the kind of [`value`](Self::value).
    #[must_use]
    #[inline]
    pub fn kind(&self) -> Kind {
        self.value.kind()
    }
}

/// A builder for constructing [`BinOverride`] instances.
///
/// # Examples
///
/// ```
/// use ltk_meta::{path::PropertyPath, property::{values, NoMeta}, BinObject, BinOverride};
///
/// let patch_bin = BinOverride::<NoMeta>::builder()
///     .delete(0xdeadbeef_u32)
///     .object(BinObject::new(0x1234, 0x5678))
///     .set(0xa4edcb0d_u32, PropertyPath::new("FlipX")?, values::Bool::new(true))
///     .build();
/// # Ok::<(), ltk_meta::path::PropertyPathError>(())
/// ```
#[derive(Debug, Clone)]
pub struct Builder<M = NoMeta> {
    deleted: Vec<BinHash>,
    objects: Vec<BinObject<M>>,
    patches: Vec<PropertyPatch<M>>,
}

impl<M> Default for Builder<M> {
    fn default() -> Self {
        Self {
            deleted: Vec::new(),
            objects: Vec::new(),
            patches: Vec::new(),
        }
    }
}

impl<M> Builder<M> {
    /// See: [`BinOverride::builder`]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks an object for deletion.
    #[must_use]
    pub fn delete(mut self, object_hash: impl Into<BinHash>) -> Self {
        self.deleted.push(object_hash.into());
        self
    }

    /// Marks multiple objects for deletion.
    #[must_use]
    pub fn deletions(mut self, hashes: impl IntoIterator<Item = impl Into<BinHash>>) -> Self {
        self.deleted.extend(hashes.into_iter().map(Into::into));
        self
    }

    /// Adds a whole object to the patch.
    #[must_use]
    pub fn object(mut self, object: BinObject<M>) -> Self {
        self.objects.push(object);
        self
    }

    /// Adds multiple whole objects to the patch.
    #[must_use]
    pub fn objects(mut self, objects: impl IntoIterator<Item = BinObject<M>>) -> Self {
        self.objects.extend(objects);
        self
    }

    /// Adds a patch record.
    #[must_use]
    pub fn set(
        self,
        object_hash: impl Into<BinHash>,
        path: PropertyPath,
        value: impl Into<PropertyValueEnum<M>>,
    ) -> Self {
        self.patch(PropertyPatch::new(object_hash, path, value))
    }

    /// Adds a patch record.
    #[must_use]
    pub fn patch(mut self, patch: PropertyPatch<M>) -> Self {
        self.patches.push(patch);
        self
    }

    /// Adds multiple patch records.
    #[must_use]
    pub fn patches(mut self, patches: impl IntoIterator<Item = PropertyPatch<M>>) -> Self {
        self.patches.extend(patches);
        self
    }

    /// Builds the final [`BinOverride`].
    #[must_use]
    pub fn build(self) -> BinOverride<M> {
        BinOverride {
            deleted: self.deleted,
            objects: self
                .objects
                .into_iter()
                .map(|object| (object.path_hash, object))
                .collect(),
            patches: self.patches,
        }
    }
}
