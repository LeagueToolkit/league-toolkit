//! [`FieldNames`]: plaintext for the hashes a [`ValuePath`](super::ValuePath) carries.

use std::{borrow::Cow, collections::HashMap, hash::BuildHasher};

use ltk_hash::BinHash;

/// Plaintext for the hashes a [`ValuePath`](super::ValuePath) carries.
///
/// A name is asked for with the class of the node the field was read on: the tables a consumer
/// holds are keyed by class, and a meta class dump names a field under its class. A table keyed
/// by field alone ignores the class. The class is the concrete class the file states. A
/// class-keyed table walks the base chain itself.
///
/// A name must hash back to its field under `BinHash::hash_str`. A rendering ignores a name that
/// does not.
///
/// # Examples
///
/// ```
/// use std::collections::HashMap;
/// use ltk_hash::{BinHash, Hash as _};
/// use ltk_meta::path::{ValueSegment, ValuePath};
///
/// let size = BinHash::hash_str("Size");
/// let names = HashMap::from([(size, "Size".to_owned())]);
/// let path: ValuePath = [ValueSegment::Field(size), ValueSegment::Index(1)].into_iter().collect();
///
/// assert_eq!(path.to_property_path(&names)?.as_str(), "Size[1]");
/// # Ok::<(), ltk_meta::path::Unnameable>(())
/// ```
pub trait FieldNames {
    /// The plaintext of `field`, if known, given the class of the node it was read on.
    ///
    /// A table keyed by field alone ignores `class`. A table keyed by class answers nothing for
    /// `None`.
    fn field(&self, field: BinHash, class: Option<BinHash>) -> Option<Cow<'_, str>>;

    /// The plaintext behind a `Hash`-kind map key, if known. The named form reads it; a client
    /// path writes the raw value.
    #[expect(
        unused_variables,
        reason = "the default names its parameter for the reader and uses none"
    )]
    fn hash(&self, hash: BinHash) -> Option<Cow<'_, str>> {
        None
    }
}

/// Names nothing: every hash renders as hex.
impl FieldNames for () {
    fn field(&self, _field: BinHash, _class: Option<BinHash>) -> Option<Cow<'_, str>> {
        None
    }
}

/// Keyed by field alone. The class is ignored.
impl<S: BuildHasher> FieldNames for HashMap<BinHash, String, S> {
    fn field(&self, field: BinHash, _class: Option<BinHash>) -> Option<Cow<'_, str>> {
        self.get(&field).map(|name| Cow::Borrowed(name.as_str()))
    }
}

/// Keyed by `(class, field)`. A field with no class is not found.
impl<S: BuildHasher> FieldNames for HashMap<(BinHash, BinHash), String, S> {
    fn field(&self, field: BinHash, class: Option<BinHash>) -> Option<Cow<'_, str>> {
        self.get(&(class?, field))
            .map(|name| Cow::Borrowed(name.as_str()))
    }
}

impl<T: FieldNames + ?Sized> FieldNames for &T {
    fn field(&self, field: BinHash, class: Option<BinHash>) -> Option<Cow<'_, str>> {
        (**self).field(field, class)
    }

    fn hash(&self, hash: BinHash) -> Option<Cow<'_, str>> {
        (**self).hash(hash)
    }
}
