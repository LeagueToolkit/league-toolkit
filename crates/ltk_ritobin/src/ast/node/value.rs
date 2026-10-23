use std::fmt::Display;

use glam::{Mat4, Vec2, Vec3, Vec4};
use ltk_hash::{BinHash, HashValue, WadHash};
use ltk_meta::PropertyKind;
use ltk_primitives::Color;

mod coerce;
pub use coerce::CanCoerce;

use crate::{
    ast::{
        hash::{HashedLiteral, Originally},
        Object,
    },
    ContainerType, RitoType, RitobinName, Spanned,
};
use crate::{span::Span, ScalarType};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "span_print", derive(crate::span::DebugSpans))]
pub enum Value {
    Unresolved {
        span: Span,
        kind: RitoType,
    },
    Unknown(Span),
    //---------------------
    None(Span),
    Bool(Spanned<bool>),
    BitBool(Spanned<bool>),
    I8(Spanned<i8>),
    U8(Spanned<u8>),
    I16(Spanned<i16>),
    U16(Spanned<u16>),
    I32(Spanned<i32>),
    U32(Spanned<u32>),
    I64(Spanned<i64>),
    U64(Spanned<u64>),
    F32(Spanned<f32>),
    Vector2(Spanned<Vec2>),
    Vector3(Spanned<Vec3>),
    Vector4(Spanned<Vec4>),
    Matrix44(Spanned<Mat4>),
    Color(Spanned<Color<u8>>),
    String(Spanned<String>), // TODO: intern this string when no escapes needed
    Hash(HashedLiteral<HashValue>),
    WadChunkLink(HashedLiteral<WadHash>),
    ObjectLink(HashedLiteral<BinHash>),
    //---------------------
    Struct(Object),
    Embedded(Object),
    Container {
        item_kind: ScalarType,
        items: Vec<Value>,
        span: Span,
    },
    UnorderedContainer {
        item_kind: ScalarType,
        items: Vec<Value>,
        span: Span,
    },
    Map(Map),
    Optional {
        item_kind: Option<ScalarType>,
        value: Option<Box<Value>>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct Map {
    pub key_kind: ScalarType,
    pub value_kind: ScalarType,
    pub entries: Vec<MapEntry>,
    pub span: Span,
}

impl Map {
    pub fn keys(&self) -> impl Iterator<Item = &Value> {
        self.entries.iter().map(|MapEntry { key, .. }| key)
    }
    pub fn values(&self) -> impl Iterator<Item = &Value> {
        self.entries
            .iter()
            .filter_map(|MapEntry { value, .. }| value.as_ref())
    }
    pub fn iter(&self) -> impl Iterator<Item = &MapEntry> {
        self.entries.iter()
    }
}

impl IntoIterator for Map {
    type Item = MapEntry;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

#[derive(Debug, Clone)]
pub struct MapEntry {
    pub key: Value,
    pub value: Option<Value>,
}

impl MapEntry {
    pub fn span(&self) -> Span {
        match self.value.as_ref() {
            Some(value) => self.key.span().cover(value.span()),
            None => self.key.span(),
        }
    }
}

impl Value {
    #[inline(always)]
    #[must_use]
    /// Whether the value is container-like - (unordered) container, map, optional
    pub fn is_containerlike(&self) -> bool {
        matches!(
            self,
            Value::Container { .. }
                | Value::UnorderedContainer { .. }
                | Value::Map { .. }
                | Value::Optional { .. }
        )
    }

    pub fn as_string(&self) -> Option<&String> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }
    pub fn into_string(self) -> Option<String> {
        match self {
            Self::String(s) => Some(s.value),
            _ => None,
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Unresolved { kind, .. } => write!(f, "unresolved {kind}"),
            Value::Unknown(_) => f.write_str("unknown"),
            Value::None(_) => f.write_str("null"),
            Value::Bool(v) => v.fmt(f),
            Value::BitBool(v) => v.fmt(f),
            Value::I8(v) => v.fmt(f),
            Value::U8(v) => v.fmt(f),
            Value::I16(v) => v.fmt(f),
            Value::U16(v) => v.fmt(f),
            Value::I32(v) => v.fmt(f),
            Value::U32(v) => v.fmt(f),
            Value::I64(v) => v.fmt(f),
            Value::U64(v) => v.fmt(f),
            Value::F32(v) => v.fmt(f),
            Value::Vector2(v) => v.fmt(f),
            Value::Vector3(v) => v.fmt(f),
            Value::Vector4(v) => v.fmt(f),
            Value::Matrix44(v) => v.fmt(f),
            Value::Color(v) => write!(f, "r: {}, g: {}, b: {}, a: {}", v.r, v.g, v.b, v.a),
            Value::String(v) => v.fmt(f),
            Value::Hash(v) => v.fmt(f),
            Value::WadChunkLink(v) => v.fmt(f),
            Value::ObjectLink(v) => v.fmt(f),
            Value::Struct(_) => f.write_str("{ ... }"),
            Value::Embedded(_) => f.write_str("{ ... }"),
            Value::Container { items, .. } | Value::UnorderedContainer { items, .. } => {
                f.write_str("[")?;
                let len = items.len();
                for (i, item) in items.iter().enumerate() {
                    item.fmt(f)?;
                    if i + 1 < len {
                        f.write_str(", ")?;
                    }
                }
                f.write_str("]")?;
                Ok(())
            }
            Value::Map { .. } => f.write_str("{ ... }"),
            Value::Optional { value, .. } => match value {
                Some(v) => v.fmt(f),
                None => f.write_str("{}"),
            },
        }
    }
}

impl Value {
    pub fn default_for(kind: RitoType, span: Span) -> Value {
        use ContainerType as C;
        use ScalarType as S;
        match kind {
            RitoType::Container(container) => match container {
                C::Map { key, value } => Value::Map(Map {
                    key_kind: key.unwrap_or_default().into(),
                    value_kind: value.unwrap_or_default().into(),
                    entries: Vec::new(),
                    span,
                }),
                C::Container(item) => Value::Container {
                    item_kind: item.unwrap_or_default().into(),
                    items: Vec::new(),
                    span,
                },
                C::UnorderedContainer(item) => Value::UnorderedContainer {
                    item_kind: item.unwrap_or_default().into(),
                    items: Vec::new(),
                    span,
                },
                C::Optional(item) => Value::Optional {
                    item_kind: item.into(),
                    value: None,
                    span,
                },
            },
            RitoType::Scalar(scalar) => match scalar {
                S::Struct => Value::Struct(Object {
                    class_hash: HashedLiteral::default()
                        .with_span(Span::new(span.start, span.start)),
                    span,
                    properties: Vec::new(),
                }),
                S::Embedded => Value::Embedded(Object {
                    class_hash: HashedLiteral::default()
                        .with_span(Span::new(span.start, span.start)),
                    span,
                    properties: Vec::new(),
                }),
                S::Hash(hash_type) => Value::Hash(HashedLiteral::new(
                    span,
                    Originally::HexLit,
                    hash_type.from_hash(0),
                )),
                S::WadChunkLink => Value::WadChunkLink(HashedLiteral::default().with_span(span)),
                S::ObjectLink => Value::ObjectLink(HashedLiteral::default().with_span(span)),
                S::None => Value::None(span),
                S::BitBool => Value::BitBool(Spanned::spanned_default(span)),
                S::Bool => Value::Bool(Spanned::spanned_default(span)),
                S::I8 => Value::I8(Spanned::spanned_default(span)),
                S::U8 => Value::U8(Spanned::spanned_default(span)),
                S::I16 => Value::I16(Spanned::spanned_default(span)),
                S::U16 => Value::U16(Spanned::spanned_default(span)),
                S::I32 => Value::I32(Spanned::spanned_default(span)),
                S::U32 => Value::U32(Spanned::spanned_default(span)),
                S::I64 => Value::I64(Spanned::spanned_default(span)),
                S::U64 => Value::U64(Spanned::spanned_default(span)),
                S::F32 => Value::F32(Spanned::spanned_default(span)),
                S::Vector2 => Value::Vector2(Spanned::spanned_default(span)),
                S::Vector3 => Value::Vector3(Spanned::spanned_default(span)),
                S::Vector4 => Value::Vector4(Spanned::spanned_default(span)),
                S::Matrix44 => Value::Matrix44(Spanned::spanned_default(span)),
                S::Color => Value::Color(Spanned::spanned_default(span)),
                S::String => Value::String(Spanned::spanned_default(span)),
            },
        }
    }
}

impl Value {
    /// `None` when we are [`Value::Unresolved`].
    pub fn kind(&self) -> Option<PropertyKind> {
        use PropertyKind as K;
        Some(match self {
            Value::Unresolved { kind, .. } => kind.base(),
            Value::Unknown(_) => return None,
            Value::None(_) => K::None,
            Value::Bool(_) => K::Bool,
            Value::BitBool(_) => K::BitBool,
            Value::I8(_) => K::I8,
            Value::U8(_) => K::U8,
            Value::I16(_) => K::I16,
            Value::U16(_) => K::U16,
            Value::I32(_) => K::I32,
            Value::U32(_) => K::U32,
            Value::I64(_) => K::I64,
            Value::U64(_) => K::U64,
            Value::F32(_) => K::F32,
            Value::Vector2(_) => K::Vector2,
            Value::Vector3(_) => K::Vector3,
            Value::Vector4(_) => K::Vector4,
            Value::Matrix44(_) => K::Matrix44,
            Value::Color(_) => K::Color,
            Value::String(_) => K::String,
            Value::Hash(_) => K::Hash,
            Value::WadChunkLink(_) => K::WadChunkLink,
            Value::ObjectLink(_) => K::ObjectLink,
            Value::Struct(_) => K::Struct,
            Value::Embedded(_) => K::Embedded,
            Value::Container { .. } => K::Container,
            Value::UnorderedContainer { .. } => K::UnorderedContainer,
            Value::Map { .. } => K::Map,
            Value::Optional { .. } => K::Optional,
        })
    }

    pub fn span(&self) -> Span {
        match self {
            Value::Unresolved { span, .. } => *span,
            Value::Unknown(span) => *span,
            Value::None(v) => *v,
            Value::Bool(v) => v.span,
            Value::BitBool(v) => v.span,
            Value::I8(v) => v.span,
            Value::U8(v) => v.span,
            Value::I16(v) => v.span,
            Value::U16(v) => v.span,
            Value::I32(v) => v.span,
            Value::U32(v) => v.span,
            Value::I64(v) => v.span,
            Value::U64(v) => v.span,
            Value::F32(v) => v.span,
            Value::Vector2(v) => v.span,
            Value::Vector3(v) => v.span,
            Value::Vector4(v) => v.span,
            Value::Matrix44(v) => v.span,
            Value::Color(v) => v.span,
            Value::String(v) => v.span,
            Value::Hash(v) => v.span(),
            Value::WadChunkLink(v) => v.span(),
            Value::ObjectLink(v) => v.span(),
            Value::Struct(s) | Value::Embedded(s) => s.span,
            Value::Container { span, .. }
            | Value::UnorderedContainer { span, .. }
            | Value::Map(Map { span, .. })
            | Value::Optional { span, .. } => *span,
        }
    }

    /// Returns the type of the value. Hash values use [`crate::HashType::default`],
    /// since values don't track the hash algo they came from.
    pub fn rito_type(&self) -> Option<RitoType> {
        Some(match self {
            Value::Container { item_kind, .. } => ContainerType::Container(Some(*item_kind)).into(),
            Value::UnorderedContainer { item_kind, .. } => {
                ContainerType::UnorderedContainer(Some(*item_kind)).into()
            }
            Value::Map(Map {
                key_kind,
                value_kind,
                ..
            }) => RitoType::map(*key_kind, *value_kind),
            Value::Optional { item_kind, .. } => ContainerType::Optional(*item_kind).into(),
            _ => RitoType::try_from_scalar(self.kind()?).ok()?,
        })
    }
}

impl Value {
    pub fn bool(span: Span, value: bool) -> Self {
        Self::Bool(Spanned::new(span, value))
    }
    pub fn bitbool(span: Span, value: bool) -> Self {
        Self::BitBool(Spanned::new(span, value))
    }
}

impl From<Map> for Value {
    fn from(value: Map) -> Self {
        Self::Map(value)
    }
}
