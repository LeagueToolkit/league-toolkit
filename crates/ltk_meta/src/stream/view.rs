//! Zero-copy views over one buffered object.
//!
//! [`ObjectStream::view`](crate::stream::ObjectStream::view) reads an object's declared byte
//! range once, into a buffer the handle reuses, and everything from there down — iteration,
//! random access, descent into nested values — is slice arithmetic over those bytes. Nothing
//! decodes until it is touched and nothing allocates until an *owned* value is asked for, so a
//! read-only consumer never pays the 96 bytes a materialized [`PropertyValueEnum`] node costs.
//!
//! The views are plain shared references: hold as many properties at once as you like, compare
//! them, go back to an earlier one. `M` rides along as a phantom parameter purely so the owned
//! escape hatches ([`PropertyView::value`]) infer without a turbofish; the borrowed data itself
//! carries no metadata.

mod value;
pub use value::{
    ContainerItems, ContainerView, MapEntries, MapView, OptionalView, StructView, ValueView,
};

use std::{fmt, marker::PhantomData};

use ltk_hash::BinHash;

use crate::{
    path::ValueShape,
    property::{Kind, NoMeta},
    stream::{
        layout::{Cursor, Numbering},
        owned, ObjectEntry,
    },
    Error, PropertyValueEnum,
};

/// `u32 size`, `u32 path_hash`, `u16 property_count` — what sits ahead of an object's
/// properties in its own byte range.
const OBJECT_HEADER: usize = 4 + 4 + 2;

/// One object's bytes, viewed in place.
///
/// Created by [`ObjectStream::view`](crate::stream::ObjectStream::view), which has already
/// walked the bytes: the declared sizes inside agree with what the counts consume, and the kind
/// numbering the view reads under is settled. Iterating costs no I/O and no allocation.
///
/// # Examples
///
/// ```no_run
/// use std::fs::File;
/// use ltk_meta::{concrete::BinStream, stream::ValueView};
///
/// let mut stream = BinStream::mount(File::open("data.bin")?)?;
/// let mut objects = stream.objects();
///
/// while let Some(mut object) = objects.next()? {
///     let view = object.view()?;
///     for property in view.properties() {
///         if let ValueView::String(text) = property?.value_view()? {
///             println!("{text}");
///         }
///     }
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct ObjectView<'a, M = NoMeta> {
    path_hash: BinHash,
    class_hash: BinHash,
    raw: &'a [u8],
    /// Positioned at the first property.
    properties: Cursor<'a>,
    property_count: u16,
    meta: PhantomData<fn() -> M>,
}

impl<'a, M> ObjectView<'a, M> {
    /// Views the object `cur` reads: its whole declared byte range, size field included,
    /// positioned at the start of it.
    pub(crate) fn new(entry: ObjectEntry, mut cur: Cursor<'a>) -> Result<Self, Error> {
        let raw = cur.rest();
        // The size and the path hash are already known from the table of contents; the count is
        // the only thing here worth reading, and reading it proves the header is present.
        cur.skip(OBJECT_HEADER - 2)?;
        let property_count = cur.u16()?;

        Ok(Self {
            path_hash: entry.path_hash,
            class_hash: entry.class_hash,
            raw,
            properties: cur,
            property_count,
            meta: PhantomData,
        })
    }

    /// The object's path hash.
    #[must_use]
    pub fn path_hash(&self) -> BinHash {
        self.path_hash
    }

    /// The object's class hash.
    #[must_use]
    pub fn class_hash(&self) -> BinHash {
        self.class_hash
    }

    /// How many properties the object declares.
    ///
    /// What the file says, which is what [`ObjectView::properties`] yields. The owned
    /// [`BinObject`](crate::BinObject) keys its properties by name hash, so for an object that
    /// declares one hash twice — which no shipped bin does — its map is the shorter of the two.
    #[must_use]
    pub fn property_count(&self) -> u16 {
        self.property_count
    }

    /// Which property-kind numbering the object was read under.
    #[must_use]
    pub fn numbering(&self) -> Numbering {
        self.properties.numbering()
    }

    /// The properties, in file order.
    ///
    /// Items are `Result` because a property header's kind byte can fail to decode.
    pub fn properties(&self) -> Properties<'a, M> {
        Properties::new(self.properties, self.property_count)
    }

    /// The property with the given name hash — an in-memory walk, no index needed.
    ///
    /// The *first* property with that hash, and the walk stops there. An object that declares
    /// the same name hash twice is addressed differently by the owned side, whose map keeps the
    /// last of them; no shipped bin does, and matching it here would cost every lookup the walk
    /// it currently exits early from.
    ///
    /// # Errors
    ///
    /// Whatever [`ObjectView::properties`] raises before it reaches the property.
    pub fn property(
        &self,
        name_hash: impl Into<BinHash>,
    ) -> Result<Option<PropertyView<'a, M>>, Error> {
        find_property(self.properties(), name_hash.into())
    }

    /// The object's raw bytes — its whole declared range, size field included.
    ///
    /// This is the range a byte-exact copy of the object covers, which is what the delta
    /// rewrite copies through for an object it does not touch.
    #[must_use]
    pub fn raw(&self) -> &'a [u8] {
        self.raw
    }
}

impl<M> fmt::Debug for ObjectView<'_, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObjectView")
            .field("path_hash", &self.path_hash)
            .field("class_hash", &self.class_hash)
            .field("property_count", &self.property_count)
            .field("bytes", &self.raw.len())
            .field("numbering", &self.numbering())
            .finish()
    }
}

/// One property: the header is decoded, the value is untouched.
pub struct PropertyView<'a, M = NoMeta> {
    name_hash: BinHash,
    kind: Kind,
    /// The value's own bytes, positioned at the start of them.
    value: Cursor<'a>,
    meta: PhantomData<fn() -> M>,
}

impl<'a, M> PropertyView<'a, M> {
    /// The property's name hash.
    #[must_use]
    pub fn name_hash(&self) -> BinHash {
        self.name_hash
    }

    /// The kind of the property's value.
    #[must_use]
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// The value's raw bytes, the `name_hash`/`kind` header excluded.
    ///
    /// Exactly what [`PropertyExt::size_no_header`](crate::traits::PropertyExt::size_no_header)
    /// measures for the owned value, and exactly what the writer emits for it.
    #[must_use]
    pub fn raw(&self) -> &'a [u8] {
        self.value.rest()
    }

    /// The value's wire shape, read from the few header bytes ahead of its body.
    ///
    /// The same [`ValueShape`] the resolver's type rule uses, filled by the rules of
    /// [`ValueShape::of`]: a container's or option's item kind, a map's key and value kinds, an
    /// embed's class. A pointer's class is deliberately not recorded.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidPropertyTypePrimitive`] if a header kind byte does not decode, or
    /// [`Error::IOError`] if the value's bytes end inside its header.
    pub fn shape(&self) -> Result<ValueShape, Error> {
        self.value.value_shape(self.kind)
    }

    /// The element count of a container or a map, from the same header bytes.
    ///
    /// `None` for every other kind, an option included: an option is not counted, and whether
    /// it holds anything is [`OptionalView::is_some`].
    ///
    /// # Errors
    ///
    /// [`Error::IOError`] if the value's bytes end inside its header.
    pub fn item_count(&self) -> Result<Option<u32>, Error> {
        let mut cur = self.value;
        let count = match self.kind {
            // The count is the first field of the sized body, past the item kind and the size.
            Kind::Container | Kind::UnorderedContainer => {
                cur.skip(1 + 4)?;
                cur.u32()?
            }
            // Same, past the key and value kinds.
            Kind::Map => {
                cur.skip(2 + 4)?;
                cur.u32()?
            }
            _ => return Ok(None),
        };
        Ok(Some(count))
    }

    /// Descends into the value without materializing it.
    ///
    /// # Errors
    ///
    /// See [`ValueView`].
    pub fn value_view(&self) -> Result<ValueView<'a, M>, Error> {
        ValueView::read(&mut { self.value }, self.kind)
    }
}

impl<M: Default> PropertyView<'_, M> {
    /// Decodes the value — the whole subtree — into the owned representation.
    ///
    /// # Errors
    ///
    /// The same as the eager reader raises for the same bytes: [`Error::InvalidSize`],
    /// [`Error::InvalidNesting`], [`Error::InvalidKeyType`],
    /// [`Error::MismatchedContainerTypes`], [`Error::Utf8Error`].
    pub fn value(&self) -> Result<PropertyValueEnum<M>, Error> {
        owned::read_value(&mut { self.value }, self.kind)
    }
}

impl<M> fmt::Debug for PropertyView<'_, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PropertyView")
            .field("name_hash", &self.name_hash)
            .field("kind", &self.kind)
            .field("bytes", &self.value.remaining())
            .finish()
    }
}

/// Iterator over the properties of an [`ObjectView`] or a [`StructView`], in file order.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct Properties<'a, M = NoMeta> {
    cur: Cursor<'a>,
    remaining: u16,
    meta: PhantomData<fn() -> M>,
}

impl<'a, M> Properties<'a, M> {
    /// Reads `count` properties from where `cur` is positioned.
    pub(crate) fn new(cur: Cursor<'a>, count: u16) -> Self {
        Self {
            cur,
            remaining: count,
            meta: PhantomData,
        }
    }
}

impl<'a, M> Iterator for Properties<'a, M> {
    type Item = Result<PropertyView<'a, M>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;

        Some(match read_property(&mut self.cur) {
            Ok(property) => Ok(property),
            Err(error) => {
                // A failed read leaves the cursor mid-property, so there is nothing sane to
                // continue from.
                self.remaining = 0;
                Err(error)
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining as usize, Some(self.remaining as usize))
    }
}

impl<M> std::iter::FusedIterator for Properties<'_, M> {}

impl<M> Clone for Properties<'_, M> {
    fn clone(&self) -> Self {
        Self {
            cur: self.cur,
            remaining: self.remaining,
            meta: PhantomData,
        }
    }
}

impl<M> fmt::Debug for Properties<'_, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Properties")
            .field("remaining", &self.remaining)
            .finish()
    }
}

/// Reads one `name_hash`/`kind`/`value` triple, advancing past the value.
fn read_property<'a, M>(cur: &mut Cursor<'a>) -> Result<PropertyView<'a, M>, Error> {
    let name_hash = cur.bin_hash()?;
    let kind = cur.kind()?;
    let value = cur.take_value(kind)?;

    Ok(PropertyView {
        name_hash,
        kind,
        value: Cursor::new(value, cur.numbering()),
        meta: PhantomData,
    })
}

/// The first property of `properties` with the given name hash.
pub(crate) fn find_property<'a, M>(
    properties: Properties<'a, M>,
    name_hash: BinHash,
) -> Result<Option<PropertyView<'a, M>>, Error> {
    for property in properties {
        let property = property?;
        if property.name_hash == name_hash {
            return Ok(Some(property));
        }
    }
    Ok(None)
}

macro_rules! copy_views {
    ($($view:ident),* $(,)?) => { $(
        // By hand rather than derived: `M` is a phantom here, so a derived `Copy` would demand
        // `M: Copy` for a field that holds nothing.
        impl<M> Clone for $view<'_, M> {
            fn clone(&self) -> Self {
                *self
            }
        }
        impl<M> Copy for $view<'_, M> {}
    )* };
}
pub(crate) use copy_views;

copy_views!(ObjectView, PropertyView);
