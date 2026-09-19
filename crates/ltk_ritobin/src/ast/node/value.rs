use std::fmt::Display;

use glam::{Mat4, Vec2, Vec3, Vec4};
use ltk_hash::{BinHash, WadHash};
use ltk_meta::PropertyKind;
use ltk_primitives::Color;

mod coerce;
pub use coerce::CanCoerce;

use crate::{
    ast::{hash::HashedLiteral, Object},
    parse::Span,
    RitoType, RitobinName, Spanned,
};

#[derive(Debug, Clone)]
pub enum Value {
    Unresolved {
        span: Span,
        kind: PropertyKind,
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
    Hash(HashedLiteral<BinHash>),
    WadChunkLink(HashedLiteral<WadHash>),
    ObjectLink(HashedLiteral<BinHash>),
    //---------------------
    Struct(Object),
    Embedded(Object),
    Container {
        item_kind: PropertyKind,
        items: Vec<Value>,
        span: Span,
    },
    UnorderedContainer {
        item_kind: PropertyKind,
        items: Vec<Value>,
        span: Span,
    },
    Map {
        key_kind: PropertyKind,
        value_kind: PropertyKind,
        entries: Vec<(Value, Option<Value>)>,
        span: Span,
    },
    Optional {
        item_kind: Option<PropertyKind>,
        value: Option<Box<Value>>,
        span: Span,
    },
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
            Value::Unresolved { kind, .. } => write!(f, "unresolved {}", kind.to_rito_name()),
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
        use PropertyKind as K;
        match kind.base {
            K::Map => Value::Map {
                key_kind: kind.subtype(0),
                value_kind: kind.subtype(1),
                entries: Vec::new(),
                span,
            },
            K::Container => Value::Container {
                item_kind: kind.subtype(0),
                items: Vec::new(),
                span,
            },
            K::UnorderedContainer => Value::UnorderedContainer {
                item_kind: kind.subtype(0),
                items: Vec::new(),
                span,
            },
            K::Optional => Value::Optional {
                item_kind: kind.subtypes[0],
                value: None,
                span,
            },
            K::Struct => Value::Struct(Object {
                class_hash: HashedLiteral::default().with_span(Span::new(span.start, span.start)),
                span,
                properties: Vec::new(),
            }),
            K::Embedded => Value::Embedded(Object {
                class_hash: HashedLiteral::default().with_span(Span::new(span.start, span.start)),
                span,
                properties: Vec::new(),
            }),
            K::Hash => Value::Hash(HashedLiteral::default().with_span(span)),
            K::WadChunkLink => Value::WadChunkLink(HashedLiteral::default().with_span(span)),
            K::ObjectLink => Value::ObjectLink(HashedLiteral::default().with_span(span)),
            K::None => Value::None(span),
            K::BitBool => Value::BitBool(Spanned::spanned_default(span)),
            K::Bool => Value::Bool(Spanned::spanned_default(span)),
            K::I8 => Value::I8(Spanned::spanned_default(span)),
            K::U8 => Value::U8(Spanned::spanned_default(span)),
            K::I16 => Value::I16(Spanned::spanned_default(span)),
            K::U16 => Value::U16(Spanned::spanned_default(span)),
            K::I32 => Value::I32(Spanned::spanned_default(span)),
            K::U32 => Value::U32(Spanned::spanned_default(span)),
            K::I64 => Value::I64(Spanned::spanned_default(span)),
            K::U64 => Value::U64(Spanned::spanned_default(span)),
            K::F32 => Value::F32(Spanned::spanned_default(span)),
            K::Vector2 => Value::Vector2(Spanned::spanned_default(span)),
            K::Vector3 => Value::Vector3(Spanned::spanned_default(span)),
            K::Vector4 => Value::Vector4(Spanned::spanned_default(span)),
            K::Matrix44 => Value::Matrix44(Spanned::spanned_default(span)),
            K::Color => Value::Color(Spanned::spanned_default(span)),
            K::String => Value::String(Spanned::spanned_default(span)),
        }
    }
}

impl Value {
    /// `None` when we are [`Value::Unresolved`].
    pub fn kind(&self) -> Option<PropertyKind> {
        use PropertyKind as K;
        Some(match self {
            Value::Unresolved { kind, .. } => *kind,
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
            | Value::Map { span, .. }
            | Value::Optional { span, .. } => *span,
        }
    }

    pub fn rito_type(&self) -> Option<RitoType> {
        Some(match self {
            Value::Container { item_kind, .. } | Value::UnorderedContainer { item_kind, .. } => {
                RitoType {
                    base: self.kind()?,
                    subtypes: [Some(*item_kind), None],
                }
            }
            Value::Map {
                key_kind,
                value_kind,
                ..
            } => RitoType {
                base: self.kind()?,
                subtypes: [Some(*key_kind), Some(*value_kind)],
            },
            Value::Optional { item_kind, .. } => RitoType {
                base: self.kind()?,
                subtypes: [*item_kind, None],
            },
            _ => RitoType::simple(self.kind()?),
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
