//! The tree the walk sees: two sealed traits, a child step, and a decoded leaf.

use std::fmt;

use glam::{Mat4, Vec2, Vec3, Vec4};
use ltk_hash::{BinHash, WadHash};
use ltk_primitives::Color;

use crate::{property::values, property::Kind, Error, PropertyValueEnum};

pub(crate) mod sealed {
    pub trait Sealed {}
}

/// A value the walk can cross.
///
/// Sealed: implemented for `&'a PropertyValueEnum<M>` and for [`ValueView<'a, M>`], and by
/// nothing else. A visitor is written against this trait and runs over either tree.
///
/// [`ValueView<'a, M>`]: crate::stream::ValueView
pub trait TreeValue<'a>: Copy + sealed::Sealed {
    /// The node type this tree's `Struct` and `Embedded` values are.
    type Node: TreeNode<'a, Value = Self>;
    /// The values inside a container, optional or map, each with the child step reaching it.
    type Children: Iterator<Item = Result<(Child<Self>, Self), Error>>;

    /// The kind this value is.
    fn kind(&self) -> Kind;

    /// Whether entering this value can reach a node.
    ///
    /// True for a `Struct` or `Embedded` whose class hash is not 0, and for a container,
    /// optional or map whose item kind [`Kind::is_node`]. An empty optional or container of a
    /// node kind answers true: it *can* hold one, and entering it costs nothing.
    ///
    /// # Errors
    ///
    /// Over a view, a header that does not decode. The owned tree never fails.
    fn holds_node(&self) -> Result<bool, Error>;

    /// This value as a node, if it is a `Struct` or `Embedded` with a class hash that is not 0.
    ///
    /// # Errors
    ///
    /// Over a view, a header that does not decode. The owned tree never fails.
    fn as_node(&self) -> Result<Option<Self::Node>, Error>;

    /// The values inside this one, with the step reaching each. Empty for a leaf and for a
    /// node: a node's contents are its properties.
    ///
    /// # Errors
    ///
    /// Over a view, a header that does not decode. The owned tree never fails.
    fn children(&self) -> Result<Self::Children, Error>;

    /// This value decoded, if it is a leaf kind. `None` for every complex kind.
    ///
    /// # Errors
    ///
    /// Over a view, a leaf that does not decode. The owned tree never fails.
    fn leaf(&self) -> Result<Option<Leaf<'a>>, Error>;

    /// The whole value, owned. Allocates; a visitor reaches for it when it needs a subtree
    /// rather than a leaf.
    ///
    /// # Errors
    ///
    /// Over a view, whatever the eager reader raises for the same bytes. The owned tree never
    /// fails.
    fn to_value(&self) -> Result<PropertyValueEnum, Error>;
}

/// A node the walk can visit: a class and properties.
///
/// Sealed: implemented for the owned tree's node, [`OwnedNode`](super::OwnedNode), and for
/// [`StructView<'a, M>`], which an object's root also views as.
///
/// [`StructView<'a, M>`]: crate::stream::StructView
pub trait TreeNode<'a>: Copy + sealed::Sealed {
    /// The value type of this tree.
    type Value: TreeValue<'a, Node = Self>;
    /// The properties in file order. A view's kind byte can fail to decode. Items are
    /// `Result`; the owned tree never fails.
    type Properties: Iterator<Item = Result<(BinHash, Self::Value), Error>>;

    /// The class hash this node carries.
    fn class_hash(&self) -> BinHash;

    /// The properties, in file order.
    fn properties(&self) -> Self::Properties;

    /// One property by field hash: the owned tree's keyed lookup, or the view's in-place
    /// scan.
    ///
    /// # Errors
    ///
    /// Over a view, a kind byte that does not decode before the property is reached. The
    /// owned tree never fails.
    fn property(&self, field: BinHash) -> Result<Option<Self::Value>, Error>;

    /// The whole node, owned, as a `Struct` carrying this class and every property. Allocates.
    /// For a root, the object's path hash is [`Node::object_hash`](super::Node::object_hash).
    ///
    /// # Errors
    ///
    /// Over a view, whatever the eager reader raises for the same bytes. The owned tree never
    /// fails.
    fn to_struct(&self) -> Result<values::Struct, Error>;
}

/// The step from a container, optional or map to one value inside it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Child<V> {
    /// A container element, or the value of a present optional (always 0).
    Index(usize),
    /// A map entry, by its key value.
    Key(V),
}

/// A leaf, decoded and borrowed.
///
/// The client's names for the tags, not the wire enum's: `File` is [`Kind::WadChunkLink`],
/// `Link` is [`Kind::ObjectLink`], `Flag` is [`Kind::BitBool`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Leaf<'a> {
    /// [`Kind::None`].
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
    /// [`Kind::Matrix44`].
    Matrix44(Mat4),
    /// [`Kind::Color`].
    Color(Color<u8>),
    /// [`Kind::String`].
    String(&'a str),
    /// [`Kind::Hash`].
    Hash(BinHash),
    /// [`Kind::WadChunkLink`].
    File(WadHash),
    /// [`Kind::ObjectLink`].
    Link(BinHash),
    /// [`Kind::BitBool`].
    Flag(bool),
}

impl Leaf<'_> {
    /// The kind this leaf is.
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
            Self::File(_) => Kind::WadChunkLink,
            Self::Link(_) => Kind::ObjectLink,
            Self::Flag(_) => Kind::BitBool,
        }
    }

    /// Writes this leaf as the text inside a `{key}` step of the hash form.
    ///
    /// An integer in decimal, a bool as `true` or `false`, a float in its shortest
    /// round-trip form, a string as a JSON string, a hash as lowercase zero-padded hex, a
    /// vector, colour or matrix as its components in parentheses, and `None` as nothing.
    pub(crate) fn write_key(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => Ok(()),
            Self::Bool(v) | Self::Flag(v) => write!(f, "{v}"),
            Self::I8(v) => write!(f, "{v}"),
            Self::U8(v) => write!(f, "{v}"),
            Self::I16(v) => write!(f, "{v}"),
            Self::U16(v) => write!(f, "{v}"),
            Self::I32(v) => write!(f, "{v}"),
            Self::U32(v) => write!(f, "{v}"),
            Self::I64(v) => write!(f, "{v}"),
            Self::U64(v) => write!(f, "{v}"),
            Self::F32(v) => write!(f, "{v}"),
            Self::Vector2(v) => write_tuple(f, &v.to_array()),
            Self::Vector3(v) => write_tuple(f, &v.to_array()),
            Self::Vector4(v) => write_tuple(f, &v.to_array()),
            Self::Matrix44(v) => write_tuple(f, &v.transpose().to_cols_array()),
            Self::Color(c) => write_tuple(f, &[c.r, c.g, c.b, c.a]),
            Self::String(s) => write_json_string(f, s),
            Self::Hash(h) | Self::Link(h) => write!(f, "{h:08x}"),
            Self::File(h) => write!(f, "{h:016x}"),
        }
    }
}

/// `(a, b, c)`.
fn write_tuple<T: fmt::Display>(f: &mut fmt::Formatter<'_>, items: &[T]) -> fmt::Result {
    f.write_str("(")?;
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            f.write_str(", ")?;
        }
        write!(f, "{item}")?;
    }
    f.write_str(")")
}

/// A JSON string literal, escaped as `serde_json` writes one: `"`, `\` and the control
/// characters.
fn write_json_string(f: &mut fmt::Formatter<'_>, s: &str) -> fmt::Result {
    f.write_str("\"")?;
    for c in s.chars() {
        match c {
            '"' => f.write_str("\\\"")?,
            '\\' => f.write_str("\\\\")?,
            '\n' => f.write_str("\\n")?,
            '\r' => f.write_str("\\r")?,
            '\t' => f.write_str("\\t")?,
            '\u{8}' => f.write_str("\\b")?,
            '\u{c}' => f.write_str("\\f")?,
            c if (c as u32) < 0x20 => write!(f, "\\u{:04x}", c as u32)?,
            c => write!(f, "{c}")?,
        }
    }
    f.write_str("\"")
}
