use std::fmt::Display;

use ltk_hash::{BinHash, WadHash};
use ltk_meta::{property::values, traits::PropertyExt as _, PropertyKind, PropertyValueEnum};
use ltk_primitives::Color;

use crate::{
    ast::{hash::HashedLiteral, AstStruct},
    parse::Span,
    RitoType, Spanned,
};

#[derive(Debug, Clone)]
pub enum AstValue {
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
    Struct(AstStruct),
    Embedded(AstStruct),
    Container {
        item_kind: PropertyKind,
        items: Vec<AstValue>,
        span: Span,
    },
    UnorderedContainer {
        item_kind: PropertyKind,
        items: Vec<AstValue>,
        span: Span,
    },
    Map {
        key_kind: PropertyKind,
        value_kind: PropertyKind,
        entries: Vec<(AstValue, AstValue)>,
        span: Span,
    },
    Optional {
        item_kind: PropertyKind,
        value: Option<Box<AstValue>>,
        span: Span,
    },
}

impl Display for AstValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AstValue::None(_) => f.write_str("null"),
            AstValue::Bool(v) => v.fmt(f),
            AstValue::BitBool(v) => v.fmt(f),
            AstValue::I8(v) => v.fmt(f),
            AstValue::U8(v) => v.fmt(f),
            AstValue::I16(v) => v.fmt(f),
            AstValue::U16(v) => v.fmt(f),
            AstValue::I32(v) => v.fmt(f),
            AstValue::U32(v) => v.fmt(f),
            AstValue::I64(v) => v.fmt(f),
            AstValue::U64(v) => v.fmt(f),
            AstValue::F32(v) => v.fmt(f),
            AstValue::Vector2(v) => v.fmt(f),
            AstValue::Vector3(v) => v.fmt(f),
            AstValue::Vector4(v) => v.fmt(f),
            AstValue::Matrix44(v) => v.fmt(f),
            AstValue::Color(v) => write!(f, "r: {}, g: {}, b: {}, a: {}", v.r, v.g, v.b, v.a),
            AstValue::String(v) => v.fmt(f),
            AstValue::Hash(v) => v.fmt(f),
            AstValue::WadChunkLink(v) => v.fmt(f),
            AstValue::ObjectLink(v) => v.fmt(f),
            AstValue::Struct(_) => f.write_str("{ ... }"),
            AstValue::Embedded(_) => f.write_str("{ ... }"),
            AstValue::Container { items, .. } | AstValue::UnorderedContainer { items, .. } => {
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
            AstValue::Map { .. } => f.write_str("{ ... }"),
            AstValue::Optional { value, .. } => match value {
                Some(v) => v.fmt(f),
                None => f.write_str("{}"),
            },
        }
    }
}

impl AstValue {
    pub fn default_for(kind: RitoType, span: Span) -> AstValue {
        use PropertyKind as K;
        match kind.base {
            K::Map => AstValue::Map {
                key_kind: kind.subtype(0),
                value_kind: kind.subtype(1),
                entries: Vec::new(),
                span,
            },
            K::Container => AstValue::Container {
                item_kind: kind.subtype(0),
                items: Vec::new(),
                span,
            },
            K::UnorderedContainer => AstValue::UnorderedContainer {
                item_kind: kind.subtype(0),
                items: Vec::new(),
                span,
            },
            K::Optional => AstValue::Optional {
                item_kind: kind.subtype(0),
                value: None,
                span,
            },
            K::Struct => AstValue::Struct(AstStruct {
                class_hash: HashedLiteral::default().with_span(Span::new(span.start, span.start)),
                span,
                properties: Vec::new(),
            }),
            K::Embedded => AstValue::Embedded(AstStruct {
                class_hash: HashedLiteral::default().with_span(Span::new(span.start, span.start)),
                span,
                properties: Vec::new(),
            }),
            K::Hash => AstValue::Hash(HashedLiteral::default().with_span(span)),
            K::WadChunkLink => AstValue::WadChunkLink(HashedLiteral::default().with_span(span)),
            K::ObjectLink => AstValue::ObjectLink(HashedLiteral::default().with_span(span)),

            other => AstValue::try_from({
                let mut v = other.default_value::<Span>();
                *v.meta_mut() = span;
                v
            }).unwrap(/* Safety: all arms that error in try_from should be handled by previous arms in this match. */),
        }
    }
}

impl TryFrom<PropertyValueEnum<Span>> for AstValue {
    type Error = ();
    fn try_from(value: PropertyValueEnum<Span>) -> Result<Self, Self::Error> {
        Ok(match value {
            PropertyValueEnum::None(values::None { meta }) => AstValue::None(meta),
            PropertyValueEnum::Bool(values::Bool { value, meta }) => {
                AstValue::Bool(Spanned::new(meta, value))
            }
            PropertyValueEnum::BitBool(values::BitBool { value, meta }) => {
                AstValue::BitBool(Spanned::new(meta, value))
            }
            PropertyValueEnum::I8(v) => AstValue::I8(v),
            PropertyValueEnum::U8(v) => AstValue::U8(v),
            PropertyValueEnum::I16(v) => AstValue::I16(v),
            PropertyValueEnum::U16(v) => AstValue::U16(v),
            PropertyValueEnum::I32(v) => AstValue::I32(v),
            PropertyValueEnum::U32(v) => AstValue::U32(v),
            PropertyValueEnum::I64(v) => AstValue::I64(v),
            PropertyValueEnum::U64(v) => AstValue::U64(v),
            PropertyValueEnum::F32(v) => AstValue::F32(v),
            PropertyValueEnum::Vector2(v) => AstValue::Vector2(v),
            PropertyValueEnum::Vector3(v) => AstValue::Vector3(v),
            PropertyValueEnum::Vector4(v) => AstValue::Vector4(v),
            PropertyValueEnum::Matrix44(v) => AstValue::Matrix44(v),
            PropertyValueEnum::Color(values::Color { value, meta }) => {
                AstValue::Color(Spanned::new(meta, value))
            }
            PropertyValueEnum::String(values::String { meta, value }) => {
                AstValue::String(Spanned::new(meta, value))
            }
            _ => return Err(()),
        })
    }
}

impl AstValue {
    pub fn kind(&self) -> PropertyKind {
        use PropertyKind as K;
        match self {
            AstValue::None(_) => K::None,
            AstValue::Bool(_) => K::Bool,
            AstValue::BitBool(_) => K::BitBool,
            AstValue::I8(_) => K::I8,
            AstValue::U8(_) => K::U8,
            AstValue::I16(_) => K::I16,
            AstValue::U16(_) => K::U16,
            AstValue::I32(_) => K::I32,
            AstValue::U32(_) => K::U32,
            AstValue::I64(_) => K::I64,
            AstValue::U64(_) => K::U64,
            AstValue::F32(_) => K::F32,
            AstValue::Vector2(_) => K::Vector2,
            AstValue::Vector3(_) => K::Vector3,
            AstValue::Vector4(_) => K::Vector4,
            AstValue::Matrix44(_) => K::Matrix44,
            AstValue::Color(_) => K::Color,
            AstValue::String(_) => K::String,
            AstValue::Hash(_) => K::Hash,
            AstValue::WadChunkLink(_) => K::WadChunkLink,
            AstValue::ObjectLink(_) => K::ObjectLink,
            AstValue::Struct(_) => K::Struct,
            AstValue::Embedded(_) => K::Embedded,
            AstValue::Container { .. } => K::Container,
            AstValue::UnorderedContainer { .. } => K::UnorderedContainer,
            AstValue::Map { .. } => K::Map,
            AstValue::Optional { .. } => K::Optional,
        }
    }

    pub fn span(&self) -> Span {
        match self {
            AstValue::None(v) => *v,
            AstValue::Bool(v) => v.span,
            AstValue::BitBool(v) => v.span,
            AstValue::I8(v) => v.meta,
            AstValue::U8(v) => v.meta,
            AstValue::I16(v) => v.meta,
            AstValue::U16(v) => v.meta,
            AstValue::I32(v) => v.meta,
            AstValue::U32(v) => v.meta,
            AstValue::I64(v) => v.meta,
            AstValue::U64(v) => v.meta,
            AstValue::F32(v) => v.meta,
            AstValue::Vector2(v) => v.meta,
            AstValue::Vector3(v) => v.meta,
            AstValue::Vector4(v) => v.meta,
            AstValue::Matrix44(v) => v.meta,
            AstValue::Color(v) => v.span,
            AstValue::String(v) => v.span,
            AstValue::Hash(v) => v.span(),
            AstValue::WadChunkLink(v) => v.span(),
            AstValue::ObjectLink(v) => v.span(),
            AstValue::Struct(s) | AstValue::Embedded(s) => s.span,
            AstValue::Container { span, .. }
            | AstValue::UnorderedContainer { span, .. }
            | AstValue::Map { span, .. }
            | AstValue::Optional { span, .. } => *span,
        }
    }

    pub fn rito_type(&self) -> RitoType {
        match self {
            AstValue::Container { item_kind, .. }
            | AstValue::UnorderedContainer { item_kind, .. } => RitoType {
                base: self.kind(),
                subtypes: [Some(*item_kind), None],
            },
            AstValue::Map {
                key_kind,
                value_kind,
                ..
            } => RitoType {
                base: self.kind(),
                subtypes: [Some(*key_kind), Some(*value_kind)],
            },
            AstValue::Optional { item_kind, .. } => RitoType {
                base: self.kind(),
                subtypes: [Some(*item_kind), None],
            },
            _ => RitoType::simple(self.kind()),
        }
    }
}

impl AstValue {
    pub fn bool(span: Span, value: bool) -> Self {
        Self::Bool(Spanned::new(span, value))
    }
    pub fn bitbool(span: Span, value: bool) -> Self {
        Self::BitBool(Spanned::new(span, value))
    }
}

impl From<values::String<Span>> for AstValue {
    fn from(values::String { value, meta }: values::String<Span>) -> Self {
        Self::String(Spanned::new(meta, value))
    }
}
