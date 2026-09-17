//! [`ValuePath`]: where a position is inside one object, by hash and by position.

mod render;
pub use render::{NamedPath, Unnameable, UnnameableKind};

#[cfg(test)]
mod tests;

use std::{
    fmt,
    hash::{Hash, Hasher},
    iter::FusedIterator,
    slice, vec,
};

use ltk_hash::{BinHash, WadHash};
use ltk_primitives::Color;

use crate::{
    property::{values, Kind},
    walk::{Leaf, TreeValue as _},
    Error, PropertyValueEnum,
};

/// Where a walk is inside one object, addressed by hash and by position.
///
/// Total: every position in a value tree has one, including a container element and a map entry.
/// A `ValuePath` is not a client path. It may name a field whose plaintext is unknown, and it is
/// never written to a file. The object it is inside is carried beside it, never in it.
///
/// Beside the segments it keeps the **class context**: for each [`ValueSegment::Field`], the class hash of
/// the node the field was read on. A name table is asked with that class. A class of 0 means
/// unknown. No node carries the null class. The context is not part of the address: two paths
/// with the same segments are equal and hash the same whatever their classes, and the hash form does
/// not print them.
///
/// # Examples
///
/// ```
/// use ltk_hash::BinHash;
/// use ltk_meta::path::{MapKey, ValuePath};
///
/// let mut path = ValuePath::new();
/// path.push_field(BinHash(0x1e6b_a0c4), BinHash(0xc1a5_0001));
/// path.push_index(3);
/// path.push_key(MapKey::String("weapon".into()));
///
/// assert_eq!(path.to_string(), r#"1e6ba0c4[3]{"weapon"}"#);
/// ```
#[derive(Clone, Debug, Default)]
pub struct ValuePath {
    segments: Vec<ValueSegment>,
    /// One class per [`ValueSegment::Field`] in `segments`, in order. 0 is unknown.
    classes: Vec<BinHash>,
}

impl ValuePath {
    /// An empty path: the object's root.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The segments, root first.
    #[must_use]
    pub fn segments(&self) -> &[ValueSegment] {
        &self.segments
    }

    /// How many segments the path holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.segments.len()
    }

    /// Whether the path is the object's root.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Appends a field segment, recording `class` in the class context.
    ///
    /// `class` is the class hash of the node `field` is on. 0 records an unknown class.
    pub fn push_field(&mut self, field: BinHash, class: BinHash) {
        self.segments.push(ValueSegment::Field(field));
        self.classes.push(class);
    }

    /// Appends an index segment: a container element, or the value of a present optional at 0.
    pub fn push_index(&mut self, index: usize) {
        self.segments.push(ValueSegment::Index(index));
    }

    /// Appends a map entry segment.
    pub fn push_key(&mut self, key: MapKey) {
        self.segments.push(ValueSegment::Key(key));
    }

    /// Appends `segment` with no class. A [`ValueSegment::Field`] records an unknown class.
    pub fn push(&mut self, segment: ValueSegment) {
        match segment {
            ValueSegment::Field(field) => self.push_field(field, BinHash(0)),
            segment => self.segments.push(segment),
        }
    }

    /// Removes the last segment, and its class if it is a field.
    pub fn pop(&mut self) -> Option<ValueSegment> {
        let segment = self.segments.pop()?;
        if let ValueSegment::Field(_) = segment {
            self.classes.pop();
        }
        Some(segment)
    }

    /// Every field segment with its class, in order. The class is `None` where it is unknown.
    pub fn fields(&self) -> Fields<'_> {
        Fields {
            segments: self.segments.iter(),
            classes: self.classes.iter(),
        }
    }
}

impl PartialEq for ValuePath {
    fn eq(&self, other: &Self) -> bool {
        self.segments == other.segments
    }
}

impl Eq for ValuePath {}

impl Hash for ValuePath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.segments.hash(state);
    }
}

/// The hash form: `.` between fields, `[i]` for an index, `{key}` for a map entry, every field
/// hash as eight lowercase hex digits. The class context is not printed.
impl fmt::Display for ValuePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, segment) in self.segments.iter().enumerate() {
            if i > 0 && matches!(segment, ValueSegment::Field(_)) {
                f.write_str(".")?;
            }
            write!(f, "{segment}")?;
        }
        Ok(())
    }
}

/// Segments only. Every field's class is unknown.
impl FromIterator<ValueSegment> for ValuePath {
    fn from_iter<I: IntoIterator<Item = ValueSegment>>(iter: I) -> Self {
        let mut path = Self::new();
        path.extend(iter);
        path
    }
}

/// Segments only. Every field's class is unknown.
impl Extend<ValueSegment> for ValuePath {
    fn extend<I: IntoIterator<Item = ValueSegment>>(&mut self, iter: I) {
        for segment in iter {
            self.push(segment);
        }
    }
}

impl<'a> IntoIterator for &'a ValuePath {
    type Item = &'a ValueSegment;
    type IntoIter = slice::Iter<'a, ValueSegment>;

    fn into_iter(self) -> Self::IntoIter {
        self.segments.iter()
    }
}

impl IntoIterator for ValuePath {
    type Item = ValueSegment;
    type IntoIter = vec::IntoIter<ValueSegment>;

    fn into_iter(self) -> Self::IntoIter {
        self.segments.into_iter()
    }
}

/// The field segments of a [`ValuePath`] with their classes: `(field, class)` pairs.
///
/// Created by [`ValuePath::fields`].
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[derive(Clone, Debug)]
pub struct Fields<'a> {
    segments: slice::Iter<'a, ValueSegment>,
    classes: slice::Iter<'a, BinHash>,
}

impl Iterator for Fields<'_> {
    type Item = (BinHash, Option<BinHash>);

    fn next(&mut self) -> Option<Self::Item> {
        let field = self.segments.find_map(|segment| match segment {
            ValueSegment::Field(field) => Some(*field),
            _ => None,
        })?;
        let class = self.classes.next().copied().filter(|class| **class != 0);
        Some((field, class))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.classes.len(), Some(self.classes.len()))
    }
}

impl ExactSizeIterator for Fields<'_> {}
impl FusedIterator for Fields<'_> {}

/// One segment from a node toward a position inside it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ValueSegment {
    /// A property of a node, by the field's name hash.
    Field(BinHash),
    /// A container element by position, or the value of a present optional, which is always 0.
    Index(usize),
    /// A map entry, by its key.
    Key(MapKey),
}

/// The segment in the hash form: a field as eight lowercase hex digits, `[i]` for an index,
/// `{key}` for a map entry. A path writes `.` before a field that follows another segment.
///
/// # Examples
///
/// ```
/// use ltk_hash::BinHash;
/// use ltk_meta::path::{MapKey, ValueSegment};
///
/// assert_eq!(ValueSegment::Field(BinHash(0x1e6b_a0c4)).to_string(), "1e6ba0c4");
/// assert_eq!(ValueSegment::Index(3).to_string(), "[3]");
/// assert_eq!(ValueSegment::Key(MapKey::U32(12)).to_string(), "{12}");
/// ```
impl fmt::Display for ValueSegment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Field(field) => write!(f, "{field:08x}"),
            Self::Index(index) => write!(f, "[{index}]"),
            Self::Key(key) => write!(f, "{{{key}}}"),
        }
    }
}

/// A map key, owned and free of metadata: every kind [`Kind::is_valid_map_key`] admits.
///
/// Floats are held as their bit patterns. The key is `Eq` and `Hash`, and two keys are equal
/// exactly when the file writes the same bytes for them. The variants carry the client's names
/// for the tags, as [`Leaf`] does.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MapKey {
    /// [`Kind::None`](crate::PropertyKind::None).
    None,
    /// [`Kind::Bool`](crate::PropertyKind::Bool).
    Bool(bool),
    /// [`Kind::I8`](crate::PropertyKind::I8).
    I8(i8),
    /// [`Kind::U8`](crate::PropertyKind::U8).
    U8(u8),
    /// [`Kind::I16`](crate::PropertyKind::I16).
    I16(i16),
    /// [`Kind::U16`](crate::PropertyKind::U16).
    U16(u16),
    /// [`Kind::I32`](crate::PropertyKind::I32).
    I32(i32),
    /// [`Kind::U32`](crate::PropertyKind::U32).
    U32(u32),
    /// [`Kind::I64`](crate::PropertyKind::I64).
    I64(i64),
    /// [`Kind::U64`](crate::PropertyKind::U64).
    U64(u64),
    /// [`Kind::F32`](crate::PropertyKind::F32).
    F32(FloatBits),
    /// [`Kind::Vector2`](crate::PropertyKind::Vector2).
    Vector2([FloatBits; 2]),
    /// [`Kind::Vector3`](crate::PropertyKind::Vector3).
    Vector3([FloatBits; 3]),
    /// [`Kind::Vector4`](crate::PropertyKind::Vector4).
    Vector4([FloatBits; 4]),
    /// [`Kind::Matrix44`](crate::PropertyKind::Matrix44), row by row as the wire holds it.
    Matrix44([FloatBits; 16]),
    /// [`Kind::Color`](crate::PropertyKind::Color).
    Color(Color<u8>),
    /// [`Kind::String`](crate::PropertyKind::String).
    String(String),
    /// [`Kind::Hash`](crate::PropertyKind::Hash).
    Hash(BinHash),
    /// [`Kind::WadChunkLink`](crate::PropertyKind::WadChunkLink).
    File(WadHash),
}

/// The key as the text inside a `{key}` segment of the hash form: an integer in decimal, a float in
/// its shortest round-trip form, a string as a JSON string, a hash as lowercase zero-padded hex, a
/// vector, colour or matrix as its components in parentheses, and `None` as nothing.
///
/// # Examples
///
/// ```
/// use ltk_hash::BinHash;
/// use ltk_meta::path::MapKey;
///
/// assert_eq!(MapKey::String("weapon".into()).to_string(), r#""weapon""#);
/// assert_eq!(MapKey::Hash(BinHash(0x1e6b_a0c4)).to_string(), "1e6ba0c4");
/// ```
impl fmt::Display for MapKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_leaf().write_key(f)
    }
}

impl MapKey {
    /// The kind of value this key is.
    #[must_use]
    pub fn kind(&self) -> Kind {
        self.as_leaf().kind()
    }

    /// A leaf as a key, or `None` for a [`Leaf::Link`] or a [`Leaf::Flag`]. No map is keyed by
    /// either.
    #[must_use]
    pub fn from_leaf(leaf: Leaf<'_>) -> Option<Self> {
        Some(match leaf {
            Leaf::None => Self::None,
            Leaf::Bool(v) => Self::Bool(v),
            Leaf::I8(v) => Self::I8(v),
            Leaf::U8(v) => Self::U8(v),
            Leaf::I16(v) => Self::I16(v),
            Leaf::U16(v) => Self::U16(v),
            Leaf::I32(v) => Self::I32(v),
            Leaf::U32(v) => Self::U32(v),
            Leaf::I64(v) => Self::I64(v),
            Leaf::U64(v) => Self::U64(v),
            Leaf::F32(v) => Self::F32(FloatBits::new(v)),
            Leaf::Vector2(v) => Self::Vector2(v.to_array().map(FloatBits::new)),
            Leaf::Vector3(v) => Self::Vector3(v.to_array().map(FloatBits::new)),
            Leaf::Vector4(v) => Self::Vector4(v.to_array().map(FloatBits::new)),
            Leaf::Matrix44(v) => Self::Matrix44(v.transpose().to_cols_array().map(FloatBits::new)),
            Leaf::Color(v) => Self::Color(v),
            Leaf::String(v) => Self::String(v.to_owned()),
            Leaf::Hash(v) => Self::Hash(v),
            Leaf::File(v) => Self::File(v),
            Leaf::Link(_) | Leaf::Flag(_) => return None,
        })
    }

    /// This key as a value of its kind.
    #[must_use]
    pub fn to_value(&self) -> PropertyValueEnum {
        match self.as_leaf() {
            Leaf::None => values::None::default().into(),
            Leaf::Bool(v) => values::Bool::new(v).into(),
            Leaf::I8(v) => values::I8::new(v).into(),
            Leaf::U8(v) => values::U8::new(v).into(),
            Leaf::I16(v) => values::I16::new(v).into(),
            Leaf::U16(v) => values::U16::new(v).into(),
            Leaf::I32(v) => values::I32::new(v).into(),
            Leaf::U32(v) => values::U32::new(v).into(),
            Leaf::I64(v) => values::I64::new(v).into(),
            Leaf::U64(v) => values::U64::new(v).into(),
            Leaf::F32(v) => values::F32::new(v).into(),
            Leaf::Vector2(v) => values::Vector2::new(v).into(),
            Leaf::Vector3(v) => values::Vector3::new(v).into(),
            Leaf::Vector4(v) => values::Vector4::new(v).into(),
            Leaf::Matrix44(v) => values::Matrix44::new(v).into(),
            Leaf::Color(v) => values::Color::new(v).into(),
            Leaf::String(v) => values::String::new(v.to_owned()).into(),
            Leaf::Hash(v) => values::Hash::new(v).into(),
            Leaf::File(v) => values::WadChunkLink::new(v).into(),
            Leaf::Link(_) | Leaf::Flag(_) => {
                unreachable!("a MapKey is never a link or a flag")
            }
        }
    }

    /// This key as the leaf it was decoded from.
    pub(crate) fn as_leaf(&self) -> Leaf<'_> {
        match self {
            Self::None => Leaf::None,
            Self::Bool(v) => Leaf::Bool(*v),
            Self::I8(v) => Leaf::I8(*v),
            Self::U8(v) => Leaf::U8(*v),
            Self::I16(v) => Leaf::I16(*v),
            Self::U16(v) => Leaf::U16(*v),
            Self::I32(v) => Leaf::I32(*v),
            Self::U32(v) => Leaf::U32(*v),
            Self::I64(v) => Leaf::I64(*v),
            Self::U64(v) => Leaf::U64(*v),
            Self::F32(v) => Leaf::F32(v.get()),
            Self::Vector2(v) => Leaf::Vector2(glam::Vec2::from_array(floats(v))),
            Self::Vector3(v) => Leaf::Vector3(glam::Vec3::from_array(floats(v))),
            Self::Vector4(v) => Leaf::Vector4(glam::Vec4::from_array(floats(v))),
            Self::Matrix44(v) => {
                Leaf::Matrix44(glam::Mat4::from_cols_array(&floats(v)).transpose())
            }
            Self::Color(v) => Leaf::Color(*v),
            Self::String(v) => Leaf::String(v),
            Self::Hash(v) => Leaf::Hash(*v),
            Self::File(v) => Leaf::File(*v),
        }
    }
}

/// Every kind [`Kind::is_valid_map_key`] admits converts. Metadata is dropped.
///
/// # Errors
///
/// [`Error::InvalidKeyType`] for a kind no map is keyed by.
impl TryFrom<&PropertyValueEnum> for MapKey {
    type Error = Error;

    fn try_from(value: &PropertyValueEnum) -> Result<Self, Error> {
        value.map_key()
    }
}

fn floats<const N: usize>(bits: &[FloatBits; N]) -> [f32; N] {
    bits.map(FloatBits::get)
}

/// An `f32` by its bits: `Eq` and `Hash`, and equal exactly when the wire bytes are.
///
/// `NaN` equals itself when the bits agree, and `-0.0` differs from `0.0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FloatBits(u32);

impl FloatBits {
    /// The bits of `value`.
    #[must_use]
    pub fn new(value: f32) -> Self {
        Self(value.to_bits())
    }

    /// The float these bits are.
    #[must_use]
    pub fn get(self) -> f32 {
        f32::from_bits(self.0)
    }
}

impl From<f32> for FloatBits {
    fn from(value: f32) -> Self {
        Self::new(value)
    }
}
