use std::fmt::Display;

use ltk_hash::{BinHash, WadHash};
use ltk_meta::{property::values, traits::PropertyExt as _, PropertyKind, PropertyValueEnum};
use ltk_primitives::Color;

mod coerce;
pub use coerce::CanCoerce;

use crate::{
    ast::{hash::HashedLiteral, Object},
    parse::Span,
    RitoType, Spanned,
};

#[derive(Debug, Clone)]
pub enum Value {
    None(Span),
    Bool(Spanned<bool>),
    BitBool(Spanned<bool>),
    I8(values::I8<Span>),
    U8(values::U8<Span>),
    I16(values::I16<Span>),
    U16(values::U16<Span>),
    I32(values::I32<Span>),
    U32(values::U32<Span>),
    I64(values::I64<Span>),
    U64(values::U64<Span>),
    F32(values::F32<Span>),
    Vector2(values::Vector2<Span>),
    Vector3(values::Vector3<Span>),
    Vector4(values::Vector4<Span>),
    Matrix44(values::Matrix44<Span>),
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
        item_kind: PropertyKind,
        value: Option<Box<Value>>,
        span: Span,
    },
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
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
                item_kind: kind.subtype(0),
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

            other => Value::try_from({
                let mut v = other.default_value::<Span>();
                *v.meta_mut() = span;
                v
            }).unwrap(/* Safety: all arms that error in try_from should be handled by previous arms in this match. */),
        }
    }
}

impl TryFrom<PropertyValueEnum<Span>> for Value {
    type Error = ();
    fn try_from(value: PropertyValueEnum<Span>) -> Result<Self, Self::Error> {
        Ok(match value {
            PropertyValueEnum::None(values::None { meta }) => Value::None(meta),
            PropertyValueEnum::Bool(values::Bool { value, meta }) => {
                Value::Bool(Spanned::new(meta, value))
            }
            PropertyValueEnum::BitBool(values::BitBool { value, meta }) => {
                Value::BitBool(Spanned::new(meta, value))
            }
            PropertyValueEnum::I8(v) => Value::I8(v),
            PropertyValueEnum::U8(v) => Value::U8(v),
            PropertyValueEnum::I16(v) => Value::I16(v),
            PropertyValueEnum::U16(v) => Value::U16(v),
            PropertyValueEnum::I32(v) => Value::I32(v),
            PropertyValueEnum::U32(v) => Value::U32(v),
            PropertyValueEnum::I64(v) => Value::I64(v),
            PropertyValueEnum::U64(v) => Value::U64(v),
            PropertyValueEnum::F32(v) => Value::F32(v),
            PropertyValueEnum::Vector2(v) => Value::Vector2(v),
            PropertyValueEnum::Vector3(v) => Value::Vector3(v),
            PropertyValueEnum::Vector4(v) => Value::Vector4(v),
            PropertyValueEnum::Matrix44(v) => Value::Matrix44(v),
            PropertyValueEnum::Color(values::Color { value, meta }) => {
                Value::Color(Spanned::new(meta, value))
            }
            PropertyValueEnum::String(values::String { meta, value }) => {
                Value::String(Spanned::new(meta, value))
            }
            _ => return Err(()),
        })
    }
}

impl Value {
    pub fn kind(&self) -> PropertyKind {
        use PropertyKind as K;
        match self {
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
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Value::None(v) => *v,
            Value::Bool(v) => v.span,
            Value::BitBool(v) => v.span,
            Value::I8(v) => v.meta,
            Value::U8(v) => v.meta,
            Value::I16(v) => v.meta,
            Value::U16(v) => v.meta,
            Value::I32(v) => v.meta,
            Value::U32(v) => v.meta,
            Value::I64(v) => v.meta,
            Value::U64(v) => v.meta,
            Value::F32(v) => v.meta,
            Value::Vector2(v) => v.meta,
            Value::Vector3(v) => v.meta,
            Value::Vector4(v) => v.meta,
            Value::Matrix44(v) => v.meta,
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

    pub fn rito_type(&self) -> RitoType {
        match self {
            Value::Container { item_kind, .. } | Value::UnorderedContainer { item_kind, .. } => {
                RitoType {
                    base: self.kind(),
                    subtypes: [Some(*item_kind), None],
                }
            }
            Value::Map {
                key_kind,
                value_kind,
                ..
            } => RitoType {
                base: self.kind(),
                subtypes: [Some(*key_kind), Some(*value_kind)],
            },
            Value::Optional { item_kind, .. } => RitoType {
                base: self.kind(),
                subtypes: [Some(*item_kind), None],
            },
            _ => RitoType::simple(self.kind()),
        }
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

impl From<values::String<Span>> for Value {
    fn from(values::String { value, meta }: values::String<Span>) -> Self {
        Self::String(Spanned::new(meta, value))
    }
}
