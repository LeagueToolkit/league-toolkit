use ltk_hash::BinHash;
use ltk_meta::{property::values, PropertyKind};

use crate::{parse::Span, RitoType};

/// How a shared unit is owned - swapped by the `salsa` feature (see the [`crate::ast`] module
/// docs). Used at exactly one place: [`crate::ast::build::AstObject::object`].
#[cfg(not(feature = "salsa"))]
pub(crate) type Ptr<T> = Box<T>;
#[cfg(feature = "salsa")]
pub(crate) type Ptr<T> = std::sync::Arc<T>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spanned<T> {
    pub span: Span,
    pub value: T,
}

impl<T> Spanned<T> {
    pub fn new(span: Span, value: T) -> Self {
        Self { span, value }
    }
}

/// A fully resolved ritobin value, retaining source spans throughout.
///
/// Unlike [`ltk_meta::PropertyValueEnum`], [`AstValue::Struct`]/[`AstValue::Embedded`] hold an
/// [`AstStruct`] rather than an `IndexMap<BinHash, _>` - that's what lets every property keep its
/// name token's own span (see [`AstProperty::name`]), not just its value's span.
#[derive(Debug, Clone)]
pub enum AstValue {
    None(values::None<Span>),
    Bool(values::Bool<Span>),
    BitBool(values::BitBool<Span>),
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
    Color(values::Color<Span>),
    String(values::String<Span>),
    Hash(values::Hash<Span>),
    WadChunkLink(values::WadChunkLink<Span>),
    ObjectLink(values::ObjectLink<Span>),
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

#[derive(Debug, Clone)]
pub struct AstStruct {
    pub class_hash: Spanned<BinHash>,
    /// Whole `ClassName { .. }` span (the CST `Class` node's own span), not just the class-name
    /// token's span - needed so span-containment descent (`Ast::locate`) can descend into a
    /// struct's own properties.
    pub span: Span,
    pub properties: Vec<AstProperty>,
}

#[derive(Debug, Clone)]
pub struct AstProperty {
    pub name: Spanned<BinHash>,
    pub type_span: Option<Span>,
    pub value: AstValue,
}

impl AstProperty {
    /// Span of the whole property, name through value - same formula as
    /// `typecheck::ir::IrItem::span()` (`key.start .. value.end.max(key.end)`), computed on
    /// demand rather than stored.
    pub fn span(&self) -> Span {
        let value_span = self.value.span();
        Span::new(self.name.span.start, value_span.end.max(self.name.span.end))
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
            AstValue::None(v) => v.meta,
            AstValue::Bool(v) => v.meta,
            AstValue::BitBool(v) => v.meta,
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
            AstValue::Color(v) => v.meta,
            AstValue::String(v) => v.meta,
            AstValue::Hash(v) => v.meta,
            AstValue::WadChunkLink(v) => v.meta,
            AstValue::ObjectLink(v) => v.meta,
            AstValue::Struct(s) | AstValue::Embedded(s) => s.span,
            AstValue::Container { span, .. }
            | AstValue::UnorderedContainer { span, .. }
            | AstValue::Map { span, .. }
            | AstValue::Optional { span, .. } => *span,
        }
    }

    /// The [`RitoType`] this value resolved to - subtypes filled in for the container-shaped
    /// variants, matching [`crate::PropertyValueExt::rito_type`]'s behavior for
    /// `PropertyValueEnum`.
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
