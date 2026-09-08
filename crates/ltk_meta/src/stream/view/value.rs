//! [`ValueView`]: the borrowed mirror of [`PropertyValueEnum`](crate::PropertyValueEnum).
//!
//! Leaves carry decoded primitives — a `&'a str` for a string, validated on the way out —
//! and complex kinds carry sub-views that descend further, still zero-copy, to any depth.
//! `Elements[3].Position` is one [`ContainerView::get`] and two [`StructView::property`] calls,
//! and no sibling of anything on that path is ever looked at.

use std::{fmt, marker::PhantomData};

use glam::{Mat4, Vec2, Vec3, Vec4};
use ltk_hash::{BinHash, WadHash};
use ltk_primitives::Color;

use crate::{
    property::{Kind, NoMeta},
    stream::{
        layout::Cursor,
        view::{copy_views, find_property, Properties, PropertyView},
    },
    Error,
};

/// A borrowed, lazily-decoded value: one variant per [`Kind`].
///
/// Read with [`PropertyView::value_view`], or from a sub-view's iterator. The owned mirror is
/// [`PropertyValueEnum`](crate::PropertyValueEnum), one call away through
/// [`PropertyView::value`].
pub enum ValueView<'a, M = NoMeta> {
    /// [`Kind::None`]: no bytes at all.
    None,
    /// [`Kind::Bool`].
    Bool(bool),
    /// [`Kind::I8`].
    I8(i8),
    /// [`Kind::U8`].
    U8(u8),
    /// [`Kind::I16`].
    I16(i16),
    /// [`Kind::U16`].
    U16(u16),
    /// [`Kind::I32`].
    I32(i32),
    /// [`Kind::U32`].
    U32(u32),
    /// [`Kind::I64`].
    I64(i64),
    /// [`Kind::U64`].
    U64(u64),
    /// [`Kind::F32`].
    F32(f32),
    /// [`Kind::Vector2`].
    Vector2(Vec2),
    /// [`Kind::Vector3`].
    Vector3(Vec3),
    /// [`Kind::Vector4`].
    Vector4(Vec4),
    /// [`Kind::Matrix44`], stored row by row on the wire.
    Matrix44(Mat4),
    /// [`Kind::Color`], four RGBA bytes.
    Color(Color<u8>),
    /// [`Kind::String`], borrowed from the buffered object.
    String(&'a str),
    /// [`Kind::Hash`].
    Hash(BinHash),
    /// [`Kind::WadChunkLink`].
    WadChunkLink(WadHash),
    /// [`Kind::ObjectLink`].
    ObjectLink(BinHash),
    /// [`Kind::BitBool`].
    BitBool(bool),
    /// [`Kind::Container`].
    Container(ContainerView<'a, M>),
    /// [`Kind::UnorderedContainer`].
    UnorderedContainer(ContainerView<'a, M>),
    /// [`Kind::Optional`].
    Optional(OptionalView<'a, M>),
    /// [`Kind::Map`].
    Map(MapView<'a, M>),
    /// [`Kind::Struct`] — a pointer, whose class hash is `0` when it is null.
    Struct(StructView<'a, M>),
    /// [`Kind::Embedded`].
    Embedded(StructView<'a, M>),
}

impl<'a, M> ValueView<'a, M> {
    /// Reads one value of `kind`, advancing `cur` past exactly that value.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidPropertyTypePrimitive`] for a kind byte that does not decode,
    /// [`Error::InvalidNesting`] or [`Error::InvalidKeyType`] for a container the value model
    /// refuses, [`Error::Utf8Error`] for a string that is not UTF-8, or [`Error::IOError`] at
    /// the end of the slice.
    pub(crate) fn read(cur: &mut Cursor<'a>, kind: Kind) -> Result<Self, Error> {
        use Kind as K;
        Ok(match kind {
            K::None => Self::None,
            K::Bool => Self::Bool(cur.bool()?),
            K::BitBool => Self::BitBool(cur.bool()?),
            K::I8 => Self::I8(cur.i8()?),
            K::U8 => Self::U8(cur.u8()?),
            K::I16 => Self::I16(cur.i16()?),
            K::U16 => Self::U16(cur.u16()?),
            K::I32 => Self::I32(cur.i32()?),
            K::U32 => Self::U32(cur.u32()?),
            K::I64 => Self::I64(cur.i64()?),
            K::U64 => Self::U64(cur.u64()?),
            K::F32 => Self::F32(cur.f32()?),
            K::Vector2 => Self::Vector2(cur.vec2()?),
            K::Vector3 => Self::Vector3(cur.vec3()?),
            K::Vector4 => Self::Vector4(cur.vec4()?),
            K::Matrix44 => Self::Matrix44(cur.mat4_row_major()?),
            K::Color => Self::Color(cur.color_u8()?),
            K::String => Self::String(cur.str_u16()?),
            K::Hash => Self::Hash(cur.bin_hash()?),
            K::WadChunkLink => Self::WadChunkLink(cur.wad_hash()?),
            K::ObjectLink => Self::ObjectLink(cur.bin_hash()?),
            K::Container => Self::Container(ContainerView::read(cur)?),
            K::UnorderedContainer => Self::UnorderedContainer(ContainerView::read(cur)?),
            K::Optional => Self::Optional(OptionalView::read(cur)?),
            K::Map => Self::Map(MapView::read(cur)?),
            K::Struct => Self::Struct(StructView::read(cur)?),
            K::Embedded => Self::Embedded(StructView::read(cur)?),
        })
    }

    /// The kind this value is.
    #[must_use]
    pub fn kind(&self) -> Kind {
        match self {
            Self::None => Kind::None,
            Self::Bool(_) => Kind::Bool,
            Self::I8(_) => Kind::I8,
            Self::U8(_) => Kind::U8,
            Self::I16(_) => Kind::I16,
            Self::U16(_) => Kind::U16,
            Self::I32(_) => Kind::I32,
            Self::U32(_) => Kind::U32,
            Self::I64(_) => Kind::I64,
            Self::U64(_) => Kind::U64,
            Self::F32(_) => Kind::F32,
            Self::Vector2(_) => Kind::Vector2,
            Self::Vector3(_) => Kind::Vector3,
            Self::Vector4(_) => Kind::Vector4,
            Self::Matrix44(_) => Kind::Matrix44,
            Self::Color(_) => Kind::Color,
            Self::String(_) => Kind::String,
            Self::Hash(_) => Kind::Hash,
            Self::WadChunkLink(_) => Kind::WadChunkLink,
            Self::ObjectLink(_) => Kind::ObjectLink,
            Self::BitBool(_) => Kind::BitBool,
            Self::Container(_) => Kind::Container,
            Self::UnorderedContainer(_) => Kind::UnorderedContainer,
            Self::Optional(_) => Kind::Optional,
            Self::Map(_) => Kind::Map,
            Self::Struct(_) => Kind::Struct,
            Self::Embedded(_) => Kind::Embedded,
        }
    }
}

impl<M> fmt::Debug for ValueView<'_, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        macro_rules! leaf {
            ($name:literal, $value:expr) => {
                f.debug_tuple($name).field($value).finish()
            };
        }
        match self {
            Self::None => f.write_str("None"),
            Self::Bool(v) => leaf!("Bool", v),
            Self::I8(v) => leaf!("I8", v),
            Self::U8(v) => leaf!("U8", v),
            Self::I16(v) => leaf!("I16", v),
            Self::U16(v) => leaf!("U16", v),
            Self::I32(v) => leaf!("I32", v),
            Self::U32(v) => leaf!("U32", v),
            Self::I64(v) => leaf!("I64", v),
            Self::U64(v) => leaf!("U64", v),
            Self::F32(v) => leaf!("F32", v),
            Self::Vector2(v) => leaf!("Vector2", v),
            Self::Vector3(v) => leaf!("Vector3", v),
            Self::Vector4(v) => leaf!("Vector4", v),
            Self::Matrix44(v) => leaf!("Matrix44", v),
            Self::Color(v) => leaf!("Color", v),
            Self::String(v) => leaf!("String", v),
            Self::Hash(v) => leaf!("Hash", v),
            Self::WadChunkLink(v) => leaf!("WadChunkLink", v),
            Self::ObjectLink(v) => leaf!("ObjectLink", v),
            Self::BitBool(v) => leaf!("BitBool", v),
            Self::Container(v) => leaf!("Container", v),
            Self::UnorderedContainer(v) => leaf!("UnorderedContainer", v),
            Self::Optional(v) => leaf!("Optional", v),
            Self::Map(v) => leaf!("Map", v),
            Self::Struct(v) => leaf!("Struct", v),
            Self::Embedded(v) => leaf!("Embedded", v),
        }
    }
}

/// A [`Kind::Container`] or [`Kind::UnorderedContainer`], viewed in place.
pub struct ContainerView<'a, M = NoMeta> {
    item_kind: Kind,
    len: u32,
    /// Positioned at the first item.
    items: Cursor<'a>,
    meta: PhantomData<fn() -> M>,
}

impl<'a, M> ContainerView<'a, M> {
    fn read(cur: &mut Cursor<'a>) -> Result<Self, Error> {
        let item_kind = cur.kind()?;
        if item_kind.is_container() {
            return Err(Error::InvalidNesting(item_kind));
        }

        let size = cur.u32()? as usize;
        let mut items = Cursor::new(cur.take(size)?, cur.numbering());
        let len = items.u32()?;

        Ok(Self {
            item_kind,
            len,
            items,
            meta: PhantomData,
        })
    }

    /// The kind every item has.
    #[must_use]
    pub fn item_kind(&self) -> Kind {
        self.item_kind
    }

    /// How many items the container declares.
    #[must_use]
    pub fn len(&self) -> u32 {
        self.len
    }

    /// Whether the container declares no items.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The items, in order.
    pub fn iter(&self) -> ContainerItems<'a, M> {
        ContainerItems {
            cur: self.items,
            remaining: self.len,
            item_kind: self.item_kind,
            meta: PhantomData,
        }
    }

    /// The item at `index`.
    ///
    /// One offset calculation for a fixed-width item kind; a walk over the items before it
    /// for a string, a struct or an embed.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidPropertyTypePrimitive`] for a kind byte that does not decode,
    /// [`Error::InvalidNesting`] or [`Error::InvalidKeyType`] for a value the model refuses,
    /// [`Error::Utf8Error`] for a string that is not UTF-8, or [`Error::IOError`] if the
    /// container's bytes end early.
    pub fn get(&self, index: u32) -> Result<Option<ValueView<'a, M>>, Error> {
        if index >= self.len {
            return Ok(None);
        }

        let mut cur = self.items;
        match self.item_kind.fixed_width() {
            // Saturating is safe because `skip` refuses a distance it cannot add to its own
            // position: an offset too large to represent lands on the end-of-slice error a
            // walk to it would have raised, rather than wrapping around to a real position.
            Some(width) => cur.skip(width.saturating_mul(index as usize))?,
            None => {
                for _ in 0..index {
                    cur.skip_value(self.item_kind)?;
                }
            }
        }

        ValueView::read(&mut cur, self.item_kind).map(Some)
    }
}

impl<'a, M> IntoIterator for &ContainerView<'a, M> {
    type Item = Result<ValueView<'a, M>, Error>;
    type IntoIter = ContainerItems<'a, M>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<M> fmt::Debug for ContainerView<'_, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContainerView")
            .field("item_kind", &self.item_kind)
            .field("len", &self.len)
            .finish()
    }
}

/// Iterator over the items of a [`ContainerView`].
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct ContainerItems<'a, M = NoMeta> {
    cur: Cursor<'a>,
    remaining: u32,
    item_kind: Kind,
    meta: PhantomData<fn() -> M>,
}

impl<'a, M> Iterator for ContainerItems<'a, M> {
    type Item = Result<ValueView<'a, M>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;

        Some(match ValueView::read(&mut self.cur, self.item_kind) {
            Ok(value) => Ok(value),
            Err(error) => {
                self.remaining = 0;
                Err(error)
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining as usize, Some(self.remaining as usize))
    }
}

impl<M> std::iter::FusedIterator for ContainerItems<'_, M> {}

impl<M> Clone for ContainerItems<'_, M> {
    fn clone(&self) -> Self {
        Self {
            cur: self.cur,
            remaining: self.remaining,
            item_kind: self.item_kind,
            meta: PhantomData,
        }
    }
}

impl<M> fmt::Debug for ContainerItems<'_, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContainerItems")
            .field("item_kind", &self.item_kind)
            .field("remaining", &self.remaining)
            .finish()
    }
}

/// A [`Kind::Map`], viewed in place.
pub struct MapView<'a, M = NoMeta> {
    key_kind: Kind,
    value_kind: Kind,
    len: u32,
    /// Positioned at the first entry.
    entries: Cursor<'a>,
    meta: PhantomData<fn() -> M>,
}

impl<'a, M> MapView<'a, M> {
    fn read(cur: &mut Cursor<'a>) -> Result<Self, Error> {
        let key_kind = cur.kind()?;
        if !key_kind.is_valid_map_key() {
            return Err(Error::InvalidKeyType(key_kind));
        }
        let value_kind = cur.kind()?;
        if value_kind.is_container() {
            return Err(Error::InvalidNesting(value_kind));
        }

        let size = cur.u32()? as usize;
        let mut entries = Cursor::new(cur.take(size)?, cur.numbering());
        let len = entries.u32()?;

        Ok(Self {
            key_kind,
            value_kind,
            len,
            entries,
            meta: PhantomData,
        })
    }

    /// The kind every key has.
    #[must_use]
    pub fn key_kind(&self) -> Kind {
        self.key_kind
    }

    /// The kind every value has.
    #[must_use]
    pub fn value_kind(&self) -> Kind {
        self.value_kind
    }

    /// How many entries the map declares.
    #[must_use]
    pub fn len(&self) -> u32 {
        self.len
    }

    /// Whether the map declares no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The entries, in file order.
    pub fn iter(&self) -> MapEntries<'a, M> {
        MapEntries {
            cur: self.entries,
            remaining: self.len,
            key_kind: self.key_kind,
            value_kind: self.value_kind,
            meta: PhantomData,
        }
    }
}

impl<'a, M> IntoIterator for &MapView<'a, M> {
    type Item = Result<(ValueView<'a, M>, ValueView<'a, M>), Error>;
    type IntoIter = MapEntries<'a, M>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<M> fmt::Debug for MapView<'_, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapView")
            .field("key_kind", &self.key_kind)
            .field("value_kind", &self.value_kind)
            .field("len", &self.len)
            .finish()
    }
}

/// Iterator over the entries of a [`MapView`].
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct MapEntries<'a, M = NoMeta> {
    cur: Cursor<'a>,
    remaining: u32,
    key_kind: Kind,
    value_kind: Kind,
    meta: PhantomData<fn() -> M>,
}

impl<'a, M> MapEntries<'a, M> {
    fn read_entry(&mut self) -> Result<(ValueView<'a, M>, ValueView<'a, M>), Error> {
        let key = ValueView::read(&mut self.cur, self.key_kind)?;
        let value = ValueView::read(&mut self.cur, self.value_kind)?;
        Ok((key, value))
    }
}

impl<'a, M> Iterator for MapEntries<'a, M> {
    type Item = Result<(ValueView<'a, M>, ValueView<'a, M>), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;

        Some(match self.read_entry() {
            Ok(entry) => Ok(entry),
            Err(error) => {
                self.remaining = 0;
                Err(error)
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining as usize, Some(self.remaining as usize))
    }
}

impl<M> std::iter::FusedIterator for MapEntries<'_, M> {}

impl<M> Clone for MapEntries<'_, M> {
    fn clone(&self) -> Self {
        Self {
            cur: self.cur,
            remaining: self.remaining,
            key_kind: self.key_kind,
            value_kind: self.value_kind,
            meta: PhantomData,
        }
    }
}

impl<M> fmt::Debug for MapEntries<'_, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapEntries")
            .field("key_kind", &self.key_kind)
            .field("value_kind", &self.value_kind)
            .field("remaining", &self.remaining)
            .finish()
    }
}

/// A [`Kind::Optional`], viewed in place.
pub struct OptionalView<'a, M = NoMeta> {
    item_kind: Kind,
    /// Positioned at the value, when there is one.
    value: Option<Cursor<'a>>,
    meta: PhantomData<fn() -> M>,
}

impl<'a, M> OptionalView<'a, M> {
    fn read(cur: &mut Cursor<'a>) -> Result<Self, Error> {
        let item_kind = cur.kind()?;
        if item_kind.is_container() {
            return Err(Error::InvalidNesting(item_kind));
        }

        let value = match cur.bool()? {
            true => Some(Cursor::new(cur.take_value(item_kind)?, cur.numbering())),
            false => None,
        };

        Ok(Self {
            item_kind,
            value,
            meta: PhantomData,
        })
    }

    /// The kind of the item, present or not — the wire declares it either way.
    #[must_use]
    pub fn item_kind(&self) -> Kind {
        self.item_kind
    }

    /// Whether a value is present.
    #[must_use]
    pub fn is_some(&self) -> bool {
        self.value.is_some()
    }

    /// Whether no value is present.
    #[must_use]
    pub fn is_none(&self) -> bool {
        self.value.is_none()
    }

    /// The contained value, if there is one.
    ///
    /// # Errors
    ///
    /// The same as [`ContainerView::get`], for the one item this holds.
    pub fn get(&self) -> Result<Option<ValueView<'a, M>>, Error> {
        match self.value {
            Some(mut value) => ValueView::read(&mut value, self.item_kind).map(Some),
            None => Ok(None),
        }
    }
}

impl<M> fmt::Debug for OptionalView<'_, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OptionalView")
            .field("item_kind", &self.item_kind)
            .field("is_some", &self.is_some())
            .finish()
    }
}

/// A [`Kind::Struct`] or [`Kind::Embedded`], viewed in place.
///
/// A pointer with a class hash of `0` is null: it has no size field and no body, so
/// [`StructView::property_count`] is `0` and the iterator is empty.
pub struct StructView<'a, M = NoMeta> {
    class_hash: BinHash,
    property_count: u16,
    /// Positioned at the first property.
    properties: Cursor<'a>,
    meta: PhantomData<fn() -> M>,
}

impl<'a, M> StructView<'a, M> {
    fn read(cur: &mut Cursor<'a>) -> Result<Self, Error> {
        let class_hash = cur.bin_hash()?;
        if *class_hash == 0 {
            return Ok(Self {
                class_hash,
                property_count: 0,
                properties: Cursor::new(&[], cur.numbering()),
                meta: PhantomData,
            });
        }

        let size = cur.u32()? as usize;
        let mut properties = Cursor::new(cur.take(size)?, cur.numbering());
        let property_count = properties.u16()?;

        Ok(Self {
            class_hash,
            property_count,
            properties,
            meta: PhantomData,
        })
    }

    /// A view over `count` properties at `properties`, carrying `class_hash`. An object's
    /// root as a struct.
    pub(crate) fn from_parts(class_hash: BinHash, count: u16, properties: Cursor<'a>) -> Self {
        Self {
            class_hash,
            property_count: count,
            properties,
            meta: PhantomData,
        }
    }

    /// The class this is an instance of, or `0` for a null pointer.
    #[must_use]
    pub fn class_hash(&self) -> BinHash {
        self.class_hash
    }

    /// How many properties it declares.
    #[must_use]
    pub fn property_count(&self) -> u16 {
        self.property_count
    }

    /// The properties, in file order.
    pub fn properties(&self) -> Properties<'a, M> {
        Properties::new(self.properties, self.property_count)
    }

    /// The property with the given name hash — an in-memory walk, no index needed.
    ///
    /// # Errors
    ///
    /// Whatever [`StructView::properties`] raises before it reaches the property.
    pub fn property(
        &self,
        name_hash: impl Into<BinHash>,
    ) -> Result<Option<PropertyView<'a, M>>, Error> {
        find_property(self.properties(), name_hash.into())
    }
}

impl<M> fmt::Debug for StructView<'_, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StructView")
            .field("class_hash", &self.class_hash)
            .field("property_count", &self.property_count)
            .finish()
    }
}

copy_views!(ValueView, ContainerView, MapView, OptionalView, StructView,);
