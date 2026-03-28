use std::fmt::{Debug, Display};

use glam::Vec4;
use indexmap::IndexMap;
use ltk_hash::fnv1a;
use ltk_meta::{
    property::values, traits::PropertyExt, Bin, BinObject, PropertyKind, PropertyValueEnum,
};
use xxhash_rust::xxh64::xxh64;

use crate::{
    cst::{self, visitor::Visit, Child, Cst, Kind, Visitor},
    parse::{Span, Token, TokenKind},
    RitobinName,
};

#[derive(Debug, Clone)]
pub enum ClassKind {
    Str(String),
    Hash(u32),
}

#[derive(Debug, Clone)]
pub struct IrEntry {
    pub key: PropertyValueEnum<Span>,
    pub value: PropertyValueEnum<Span>,
}

#[derive(Debug, Clone)]
pub struct IrListItem(pub PropertyValueEnum<Span>);

#[derive(Debug, Clone)]
pub enum IrItem {
    Entry(IrEntry),
    ListItem(IrListItem),
}

impl IrItem {
    pub fn is_entry(&self) -> bool {
        matches!(self, Self::Entry { .. })
    }

    pub fn as_entry(&self) -> Option<&IrEntry> {
        match self {
            IrItem::Entry(i) => Some(i),
            _ => None,
        }
    }
    pub fn is_list_item(&self) -> bool {
        matches!(self, Self::ListItem { .. })
    }
    pub fn as_list_item(&self) -> Option<&IrListItem> {
        match self {
            IrItem::ListItem(i) => Some(i),
            _ => None,
        }
    }
    pub fn value(&self) -> &PropertyValueEnum<Span> {
        match self {
            IrItem::Entry(i) => &i.value,
            IrItem::ListItem(i) => &i.0,
        }
    }
    pub fn value_mut(&mut self) -> &mut PropertyValueEnum<Span> {
        match self {
            IrItem::Entry(i) => &mut i.value,
            IrItem::ListItem(i) => &mut i.0,
        }
    }
    pub fn into_value(self) -> PropertyValueEnum<Span> {
        match self {
            IrItem::Entry(i) => i.value,
            IrItem::ListItem(i) => i.0,
        }
    }
}

pub struct TypeChecker<'a> {
    ctx: Ctx<'a>,
    pub root: IndexMap<String, PropertyValueEnum<Span>>,
    // current: Option<(PropertyValueEnum, PropertyValueEnum)>,
    stack: Vec<(u32, IrItem)>,
    list_queue: Vec<IrListItem>,
    depth: u32,
}

impl<'a> TypeChecker<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            ctx: Ctx {
                text,
                diagnostics: Vec::new(),
            },
            root: IndexMap::new(),
            stack: Vec::new(),
            list_queue: Vec::new(),
            depth: 0,
        }
    }
    pub fn into_parts(
        self,
    ) -> (
        IndexMap<String, PropertyValueEnum<Span>>,
        Vec<DiagnosticWithSpan>,
    ) {
        (self.root, self.ctx.diagnostics)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RitoTypeOrVirtual {
    RitoType(RitoType),
    Numeric,
    StructOrEmbedded,
}

impl RitoTypeOrVirtual {
    pub fn numeric() -> Self {
        Self::Numeric
    }
}

impl From<RitoType> for RitoTypeOrVirtual {
    fn from(value: RitoType) -> Self {
        RitoTypeOrVirtual::RitoType(value)
    }
}

impl Display for RitoTypeOrVirtual {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RitoType(rito_type) => Display::fmt(rito_type, f),
            Self::Numeric => f.write_str("numeric type"),
            Self::StructOrEmbedded => f.write_str("struct/embedded"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ColorOrVec {
    Color,
    Vec2,
    Vec3,
    Vec4,
    Mat44,
}

#[derive(Debug, Clone, Copy)]
pub enum Diagnostic {
    MissingTree(cst::Kind),
    EmptyTree(cst::Kind),
    MissingToken(TokenKind),
    UnknownType(Span),
    MissingType(Span),

    MissingEntriesMap,
    InvalidEntriesMap {
        span: Span,
        got: RitoType,
    },
    InvalidDependenciesEntry {
        span: Span,
        got: RitoType,
    },

    TypeMismatch {
        span: Span,
        expected: RitoType,
        expected_span: Option<Span>,
        got: RitoTypeOrVirtual,
    },

    UnexpectedContainerItem {
        span: Span,
        expected: RitoType,
        expected_span: Option<Span>,
    },

    ResolveLiteral,
    AmbiguousNumeric(Span),

    NotEnoughItems {
        span: Span,
        got: u8,
        expected: ColorOrVec,
    },
    TooManyItems {
        span: Span,
        extra: u8,
        expected: ColorOrVec,
    },

    RootNonEntry,
    ShadowedEntry {
        shadowee: Span,
        shadower: Span,
    },

    InvalidHash(Span),

    SubtypeCountMismatch {
        span: Span,
        got: u8,
        expected: u8,
    },
    /// Subtypes found on a type that has no subtypes
    UnexpectedSubtypes {
        span: Span,
        base_type: Span,
    },
}

impl Diagnostic {
    pub fn span(&self) -> Option<&Span> {
        match self {
            MissingTree(_) | EmptyTree(_) | MissingToken(_) | RootNonEntry | ResolveLiteral
            | MissingEntriesMap => None,
            UnknownType(span)
            | SubtypeCountMismatch { span, .. }
            | UnexpectedSubtypes { span, .. }
            | UnexpectedContainerItem { span, .. }
            | MissingType(span)
            | TypeMismatch { span, .. }
            | ShadowedEntry { shadower: span, .. }
            | InvalidHash(span)
            | AmbiguousNumeric(span)
            | NotEnoughItems { span, .. }
            | TooManyItems { span, .. }
            | InvalidDependenciesEntry { span, .. }
            | InvalidEntriesMap { span, .. } => Some(span),
        }
    }

    pub fn default_span(self, span: Span) -> DiagnosticWithSpan {
        DiagnosticWithSpan {
            span: self.span().copied().unwrap_or(span),
            diagnostic: self,
        }
    }

    pub fn unwrap(self) -> DiagnosticWithSpan {
        DiagnosticWithSpan {
            span: self.span().copied().unwrap(),
            diagnostic: self,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DiagnosticWithSpan {
    pub diagnostic: Diagnostic,
    pub span: Span,
}

#[derive(Debug, Clone, Copy)]
pub struct MaybeSpanDiag {
    pub diagnostic: Diagnostic,
    pub span: Option<Span>,
}

impl MaybeSpanDiag {
    pub fn fallback(self, span: Span) -> DiagnosticWithSpan {
        DiagnosticWithSpan {
            span: self.span.unwrap_or(span),
            diagnostic: self.diagnostic,
        }
    }
}

impl From<Diagnostic> for MaybeSpanDiag {
    fn from(diagnostic: Diagnostic) -> Self {
        Self {
            span: diagnostic.span().copied(),
            diagnostic,
        }
    }
}

use Diagnostic::*;

pub trait PropertyValueExt {
    fn rito_type(&self) -> RitoType;
}
impl<M> PropertyValueExt for PropertyValueEnum<M> {
    fn rito_type(&self) -> RitoType {
        let base = self.kind();
        let subtypes = match self {
            PropertyValueEnum::Map(map) => [Some(map.key_kind()), Some(map.value_kind())],
            PropertyValueEnum::UnorderedContainer(values::UnorderedContainer(container))
            | PropertyValueEnum::Container(container) => [Some(container.item_kind()), None],
            PropertyValueEnum::Optional(optional) => [Some(optional.item_kind()), None],

            _ => [None, None],
        };
        RitoType { base, subtypes }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RitoType {
    pub base: PropertyKind,
    pub subtypes: [Option<PropertyKind>; 2],
}

impl Display for RitoType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let base = self.base.to_rito_name();
        match self.subtypes {
            [None, None] => f.write_str(base),
            [Some(a), None] => write!(f, "{base}[{}]", a.to_rito_name()),
            [Some(a), Some(b)] => {
                write!(f, "{base}[{},{}]", a.to_rito_name(), b.to_rito_name())
            }
            _ => write!(f, "{base}[!!]"),
        }
    }
}

impl RitoType {
    pub fn simple(kind: PropertyKind) -> Self {
        Self {
            base: kind,
            subtypes: [None, None],
        }
    }

    fn subtype(&self, idx: usize) -> PropertyKind {
        self.subtypes[idx].unwrap_or_default()
    }

    fn value_subtype(&self) -> Option<PropertyKind> {
        self.subtypes[1].or(self.subtypes[0])
    }

    pub fn make_default(&self, span: Span) -> PropertyValueEnum<Span> {
        let mut value = match self.base {
            PropertyKind::Map => {
                PropertyValueEnum::Map(values::Map::empty(self.subtype(0), self.subtype(1)))
            }
            PropertyKind::UnorderedContainer => {
                PropertyValueEnum::UnorderedContainer(values::UnorderedContainer(
                    values::Container::empty(self.subtype(0)).unwrap_or_default(),
                ))
            }
            PropertyKind::Container => PropertyValueEnum::Container(
                values::Container::empty(self.subtype(0)).unwrap_or_default(),
            ),
            PropertyKind::Optional => PropertyValueEnum::Optional(
                values::Optional::empty(self.subtype(0)).unwrap_or_default(),
            ),

            _ => self.base.default_value(),
        };
        *value.meta_mut() = span;
        value
    }
}
pub enum Statement {
    KeyValue {
        key: Span,
        value: Span,
        kind: Option<RitoType>,
    },
}

trait TreeIterExt<'a>: Iterator {
    fn expect_tree(&mut self, kind: cst::Kind) -> Result<&'a Cst, Diagnostic>;
    fn expect_token(&mut self, kind: TokenKind) -> Result<&'a Token, Diagnostic>;
}

impl<'a, I> TreeIterExt<'a> for I
where
    I: Iterator<Item = &'a cst::Child>,
{
    fn expect_tree(&mut self, kind: cst::Kind) -> Result<&'a Cst, Diagnostic> {
        self.find_map(|c| c.tree().filter(|t| t.kind == kind))
            .ok_or(MissingTree(kind))
    }
    fn expect_token(&mut self, kind: TokenKind) -> Result<&'a Token, Diagnostic> {
        self.find_map(|c| c.token().filter(|t| t.kind == kind))
            .ok_or(MissingToken(kind))
    }
}

pub struct Ctx<'a> {
    text: &'a str,
    diagnostics: Vec<DiagnosticWithSpan>,
}

pub fn coerce_type<M: Debug>(
    value: PropertyValueEnum<M>,
    to: PropertyKind,
) -> Option<PropertyValueEnum<M>> {
    match to {
        PropertyKind::Hash => Some(match value {
            PropertyValueEnum::Hash(_) => return Some(value),
            PropertyValueEnum::String(str) => {
                values::Hash::new_with_meta(fnv1a::hash_lower(&str), str.meta).into()
            }
            other => {
                eprintln!("\x1b[41mcannot coerce {other:?} to {to:?}\x1b[0m");
                return None;
            }
        }),
        PropertyKind::ObjectLink => Some(match value {
            PropertyValueEnum::Hash(hash) => {
                values::ObjectLink::new_with_meta(*hash, hash.meta).into()
            }
            PropertyValueEnum::ObjectLink(_) => return Some(value),
            PropertyValueEnum::String(str) => {
                values::ObjectLink::new_with_meta(fnv1a::hash_lower(&str), str.meta).into()
            }
            other => {
                eprintln!("\x1b[41mcannot coerce {other:?} to {to:?}\x1b[0m");
                return None;
            }
        }),
        PropertyKind::WadChunkLink => Some(match value {
            PropertyValueEnum::WadChunkLink(_) => return Some(value),
            PropertyValueEnum::Hash(hash) => {
                values::WadChunkLink::new_with_meta((*hash).into(), hash.meta).into()
            }
            PropertyValueEnum::String(str) => {
                values::WadChunkLink::new_with_meta(xxh64(str.as_bytes(), 0), str.meta).into()
            }
            other => {
                eprintln!("\x1b[41mcannot coerce {other:?} to {to:?}\x1b[0m");
                return None;
            }
        }),
        PropertyKind::BitBool => Some(match value {
            PropertyValueEnum::BitBool(_) => return Some(value),
            PropertyValueEnum::Bool(bool) => {
                values::BitBool::new_with_meta(*bool, bool.meta).into()
            }
            other => {
                eprintln!("\x1b[41mcannot coerce {other:?} to {to:?}\x1b[0m");
                return None;
            }
        }),
        PropertyKind::Bool => Some(match value {
            PropertyValueEnum::Bool(_) => return Some(value),
            PropertyValueEnum::BitBool(bool) => {
                values::Bool::new_with_meta(*bool, bool.meta).into()
            }
            other => {
                eprintln!("\x1b[41mcannot coerce {other:?} to {to:?}\x1b[0m");
                return None;
            }
        }),
        to if to == value.kind() => Some(value),
        _ => None,
    }
}

pub fn resolve_rito_type(ctx: &mut Ctx<'_>, tree: &Cst) -> Result<RitoType, Diagnostic> {
    let mut c = tree.children.iter();

    let base = c.expect_token(TokenKind::Name)?;
    let base_span = base.span;

    let base = PropertyKind::from_rito_name(&ctx.text[base.span]).ok_or(UnknownType(base.span))?;

    let subtypes = match c
        .clone()
        .find_map(|c| c.tree().filter(|t| t.kind == Kind::TypeArgList))
    {
        Some(subtypes) => {
            let subtypes_span = subtypes.span;

            let expected = base.subtype_count();

            if expected == 0 {
                return Err(UnexpectedSubtypes {
                    span: subtypes_span,
                    base_type: base_span,
                });
            }

            let subtypes = subtypes
                .children
                .iter()
                .filter_map(|c| c.tree().filter(|t| t.kind == Kind::TypeArg))
                .map(|t| {
                    let resolved = PropertyKind::from_rito_name(&ctx.text[t.span]);
                    if resolved.is_none() {
                        ctx.diagnostics.push(UnknownType(t.span).unwrap());
                    }
                    (resolved, t.span)
                })
                .collect::<Vec<_>>();

            if subtypes.len() > expected.into() {
                return Err(SubtypeCountMismatch {
                    span: subtypes[expected as _..]
                        .iter()
                        .map(|s| s.1)
                        .reduce(|acc, s| Span::new(acc.start, s.end))
                        .unwrap_or(subtypes_span),
                    got: subtypes.len() as u8,
                    expected,
                });
            }
            if subtypes.len() < expected.into() {
                return Err(SubtypeCountMismatch {
                    span: subtypes.last().map(|s| s.1).unwrap_or(subtypes_span),
                    got: subtypes.len() as u8,
                    expected,
                });
            }

            let mut subtypes = subtypes.iter();
            [
                subtypes.next().and_then(|s| s.0),
                subtypes.next().and_then(|s| s.0),
            ]
        }
        None => [None, None],
    };

    Ok(RitoType { base, subtypes })
}

fn resolve_hash(ctx: &Ctx, span: Span) -> Result<PropertyValueEnum<Span>, Diagnostic> {
    // TODO: better errs here?
    let src = ctx.text[span].strip_prefix("0x").ok_or(InvalidHash(span))?;

    Ok(match u32::from_str_radix(src, 16) {
        Ok(hash) => PropertyValueEnum::Hash(values::Hash::new_with_meta(hash, span)),
        Err(_) => match u64::from_str_radix(src, 16) {
            Ok(hash) => {
                PropertyValueEnum::WadChunkLink(values::WadChunkLink::new_with_meta(hash, span))
            }
            Err(_) => return Err(InvalidHash(span)),
        },
    })
}

pub fn resolve_value(
    ctx: &mut Ctx,
    tree: &Cst,
    kind_hint: Option<PropertyKind>,
) -> Result<Option<PropertyValueEnum<Span>>, Diagnostic> {
    use PropertyKind as K;
    use PropertyValueEnum as P;

    // dbg!(tree, kind_hint);

    let Some(child) = tree.children.first() else {
        return Ok(None);
    };
    Ok(Some(match child {
        cst::Child::Tree(Cst {
            kind: Kind::Class,
            children,
            span,
            ..
        }) => {
            let Some(kind_hint) = kind_hint else {
                return Ok(None); // TODO: err
            };
            let Some(class) = children.first().and_then(|t| t.token()) else {
                return Err(InvalidHash(*span));
            };

            let class_hash = match class {
                Token {
                    kind: TokenKind::Name,
                    span,
                } => fnv1a::hash_lower(&ctx.text[span]),
                Token {
                    kind: TokenKind::HexLit,
                    span,
                } => match resolve_hash(ctx, *span)? {
                    PropertyValueEnum::Hash(hash) => *hash,
                    value => {
                        return Err(TypeMismatch {
                            span: *value.meta(),
                            expected: RitoType::simple(PropertyKind::Hash),
                            expected_span: None,
                            got: value.rito_type().into(),
                        });
                    }
                },
                _ => {
                    return Err(InvalidHash(class.span));
                }
            };
            match kind_hint {
                K::Struct => P::Struct(values::Struct {
                    class_hash,
                    meta: class.span,
                    properties: Default::default(),
                }),
                K::Embedded => P::Embedded(values::Embedded(values::Struct {
                    class_hash,
                    meta: class.span,
                    properties: Default::default(),
                })),
                other => {
                    eprintln!("can't create class value from kind {other:?}");
                    return Err(TypeMismatch {
                        span: class.span,
                        expected: RitoType::simple(other),
                        expected_span: None,
                        got: RitoTypeOrVirtual::StructOrEmbedded,
                    });
                }
            }
        }
        cst::Child::Tree(Cst {
            kind: Kind::Literal,
            children,
            ..
        }) => {
            let Some(child) = children.first() else {
                return Ok(None);
            };
            match child {
                cst::Child::Token(Token {
                    kind: TokenKind::String,
                    span,
                }) => values::String::new_with_meta(
                    ctx.text[Span::new(span.start + 1, span.end - 1)].into(),
                    *span,
                )
                .into(),

                cst::Child::Token(Token {
                    kind: TokenKind::True,
                    span,
                }) => values::Bool::new_with_meta(true, *span).into(),
                cst::Child::Token(Token {
                    kind: TokenKind::False,
                    span,
                }) => values::Bool::new_with_meta(false, *span).into(),

                cst::Child::Token(Token {
                    kind: TokenKind::HexLit,
                    span,
                }) => resolve_hash(ctx, *span)?,
                cst::Child::Token(Token {
                    kind: TokenKind::Number,
                    span,
                }) => {
                    let txt = &ctx.text[span];
                    let Some(kind_hint) = kind_hint else {
                        return Err(AmbiguousNumeric(*span));
                    };

                    let txt = txt.replace('_', "");

                    match kind_hint {
                        K::U8 => P::U8(values::U8::new_with_meta(
                            txt.parse::<u8>().map_err(|_| Diagnostic::ResolveLiteral)?,
                            *span,
                        )),
                        K::U16 => P::U16(values::U16::new_with_meta(
                            txt.parse().map_err(|_| Diagnostic::ResolveLiteral)?,
                            *span,
                        )),
                        K::U32 => P::U32(values::U32::new_with_meta(
                            txt.parse().map_err(|_| Diagnostic::ResolveLiteral)?,
                            *span,
                        )),
                        K::U64 => P::U64(values::U64::new_with_meta(
                            txt.parse().map_err(|_| Diagnostic::ResolveLiteral)?,
                            *span,
                        )),
                        K::I8 => P::I8(values::I8::new_with_meta(
                            txt.parse().map_err(|_| Diagnostic::ResolveLiteral)?,
                            *span,
                        )),
                        K::I16 => P::I16(values::I16::new_with_meta(
                            txt.parse().map_err(|_| Diagnostic::ResolveLiteral)?,
                            *span,
                        )),
                        K::I32 => P::I32(values::I32::new_with_meta(
                            txt.parse().map_err(|_| Diagnostic::ResolveLiteral)?,
                            *span,
                        )),
                        K::I64 => P::I64(values::I64::new_with_meta(
                            txt.parse().map_err(|_| Diagnostic::ResolveLiteral)?,
                            *span,
                        )),
                        K::F32 => P::F32(values::F32::new_with_meta(
                            txt.parse().map_err(|_| Diagnostic::ResolveLiteral)?,
                            *span,
                        )),
                        _ => {
                            return Err(TypeMismatch {
                                span: *span,
                                expected: RitoType::simple(kind_hint),
                                expected_span: None, // TODO: would be nice here
                                got: RitoTypeOrVirtual::numeric(),
                            });
                        }
                    }
                }
                _ => return Ok(None),
            }
        }
        _ => return Ok(None),
    }))
}

pub fn resolve_entry(
    ctx: &mut Ctx,
    tree: &Cst,
    parent_value_kind: Option<RitoType>,
) -> Result<IrEntry, MaybeSpanDiag> {
    let mut c = tree.children.iter();

    let key = c.expect_tree(Kind::EntryKey)?;

    let key = match key.children.first().ok_or(InvalidHash(key.span))? {
        Child::Token(Token {
            kind: TokenKind::Name,
            span,
        }) => PropertyValueEnum::from(values::String::new_with_meta(ctx.text[span].into(), *span)),
        Child::Token(Token {
            kind: TokenKind::String,
            span,
        }) => PropertyValueEnum::from(values::String::new_with_meta(
            ctx.text[Span::new(span.start + 1, span.end - 1)].into(),
            *span,
        )),
        Child::Token(Token {
            kind: TokenKind::HexLit,
            span,
        }) => resolve_hash(ctx, *span)?,
        _ => {
            return Err(InvalidHash(key.span).into());
        }
    };

    let parent_value_kind = parent_value_kind
        .and_then(|p| p.value_subtype())
        .map(RitoType::simple);

    let kind = c
        .clone()
        .find_map(|c| c.tree().filter(|t| t.kind == Kind::TypeExpr));
    let kind_span = kind.map(|k| k.span);
    let kind = kind.map(|t| resolve_rito_type(ctx, t)).transpose()?;

    let value = c.expect_tree(Kind::EntryValue)?;
    let value_span = value.span;

    // entries: map[string, u8] = {
    //     "bad": string = "string"
    //              ^
    // }
    if let Some(parent) = parent_value_kind.as_ref() {
        if let Some((kind, kind_span)) = kind.as_ref().zip(kind_span) {
            if parent != kind {
                ctx.diagnostics.push(
                    TypeMismatch {
                        span: kind_span,
                        expected: *parent,
                        expected_span: None, // TODO: would be nice here
                        got: (*kind).into(),
                    }
                    .unwrap(),
                );
                return Ok(IrEntry {
                    key,
                    value: parent.make_default(value.span),
                });
            }
        }
    }

    let kind = kind.or(parent_value_kind);

    let resolved_val = resolve_value(ctx, value, kind.map(|k| k.base))?.map(|value| match kind {
        Some(kind) => coerce_type(value.clone(), kind.base).unwrap_or(value),
        None => value,
    });

    let value = match (kind, resolved_val) {
        (None, Some(value)) => value,
        (None, None) => return Err(MissingType(*key.meta()).into()),
        (Some(kind), Some(ivalue)) => match ivalue.kind() == kind.base {
            true => ivalue,
            false => {
                return Err(TypeMismatch {
                    span: *ivalue.meta(),
                    expected: kind,
                    expected_span: kind_span,
                    got: ivalue.rito_type().into(),
                }
                .into())
            }
        },
        (Some(kind), _) => kind.make_default(value_span),
    };

    Ok(IrEntry { key, value })
}

impl<'a> TypeChecker<'a> {
    pub fn collect_to_bin(mut self) -> (Bin, Vec<DiagnosticWithSpan>) {
        let dependencies = self.root.swap_remove("linked").and_then(|v| {
            let PropertyValueEnum::Container(list) = v else {
                self.ctx.diagnostics.push(
                    InvalidDependenciesEntry {
                        span: *v.meta(),
                        got: RitoType::simple(v.kind()),
                    }
                    .unwrap(),
                );
                return None;
            };

            Some(
                list.into_items()
                    .filter_map(|value| {
                        let span = *value.meta();
                        let PropertyValueEnum::String(dependency) =
                            coerce_type(value, PropertyKind::String)?
                        else {
                            self.ctx.diagnostics.push(
                                UnexpectedContainerItem {
                                    span,
                                    expected: RitoType::simple(PropertyKind::String),
                                    expected_span: None,
                                }
                                .unwrap(),
                            );
                            return None;
                        };
                        Some(dependency.value)
                    })
                    .collect::<Vec<_>>(),
            )
        });

        let objects = self
            .root
            .swap_remove("entries")
            .and_then(|v| {
                let PropertyValueEnum::Map(map) = v else {
                    self.ctx.diagnostics.push(
                        InvalidEntriesMap {
                            span: *v.meta(),
                            got: RitoType::simple(v.kind()),
                        }
                        .unwrap(),
                    );
                    return None;
                };
                Some(
                    map.into_entries()
                        .into_iter()
                        .filter_map(|(key, value)| {
                            let PropertyValueEnum::Hash(path_hash) =
                                coerce_type(key, PropertyKind::Hash)?
                            else {
                                return None;
                            };

                            if let PropertyValueEnum::Embedded(values::Embedded(struct_val)) = value
                            {
                                let struct_val = struct_val.no_meta();
                                // eprintln!("struct_val: {struct_val:?}");
                                Some(BinObject {
                                    path_hash: *path_hash,
                                    class_hash: struct_val.class_hash,
                                    properties: struct_val.properties.clone(),
                                })
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .unwrap_or_default();

        let tree = Bin::new(objects, dependencies.unwrap_or_default());

        (tree, self.ctx.diagnostics)
    }

    fn merge_ir(&mut self, mut parent: IrItem, child: IrItem) -> IrItem {
        // eprintln!("\x1b[0;33mmerge {child:?}\n-----> {parent:?}\x1b[0m");
        match &mut parent.value_mut() {
            PropertyValueEnum::Container(list)
            | PropertyValueEnum::UnorderedContainer(values::UnorderedContainer(list)) => {
                match child {
                    IrItem::ListItem(IrListItem(value)) => {
                        let value = coerce_type(value.clone(), list.item_kind()).unwrap_or(value);
                        let span = *value.meta();
                        match list.push(value) {
                            Ok(_) => {}
                            Err(ltk_meta::Error::MismatchedContainerTypes { expected, got }) => {
                                self.ctx.diagnostics.push(
                                    TypeMismatch {
                                        span,
                                        expected: RitoType::simple(expected),
                                        expected_span: None, // TODO: would be nice here
                                        got: RitoType::simple(got).into(),
                                    }
                                    .unwrap(),
                                );
                            }
                            Err(_e) => {
                                todo!("handle unexpected error");
                            }
                        }
                    }
                    IrItem::Entry(IrEntry { key: _, value: _ }) => {
                        eprintln!("\x1b[41mlist item must be list item\x1b[0m");
                        return parent;
                    }
                }
            }
            PropertyValueEnum::Struct(struct_val)
            | PropertyValueEnum::Embedded(values::Embedded(struct_val)) => {
                let IrItem::Entry(IrEntry { key, value }) = child else {
                    eprintln!("\x1b[41mstruct item must be entry\x1b[0m");
                    return parent;
                };

                let Some(PropertyValueEnum::Hash(key)) = coerce_type(key, PropertyKind::Hash)
                else {
                    // eprintln!("\x1b[41m{other:?} not valid hash\x1b[0m");
                    return parent;
                };

                struct_val.properties.insert(*key, value);
            }
            PropertyValueEnum::Map(map_value) => {
                let IrItem::Entry(IrEntry { key, value }) = child else {
                    eprintln!("map item must be entry");
                    return parent;
                };
                let span = *value.meta();
                let Some(key) = coerce_type(key, map_value.key_kind()) else {
                    // eprintln!("\x1b[41m{other:?} not valid hash\x1b[0m");
                    return parent;
                };
                match map_value.push(key, value) {
                    Ok(()) => {}
                    Err(ltk_meta::Error::MismatchedContainerTypes { expected, got }) => {
                        self.ctx.diagnostics.push(
                            TypeMismatch {
                                span,
                                expected: RitoType::simple(expected),
                                expected_span: None, // TODO: would be nice here
                                got: RitoType::simple(got).into(),
                            }
                            .unwrap(),
                        );
                    }
                    Err(_e) => {
                        todo!("handle unexpected err");
                    }
                }
            }
            PropertyValueEnum::Optional(option) => {
                let IrItem::ListItem(IrListItem(child)) = child else {
                    eprintln!("\x1b[41moptional value must be list item\x1b[0m");
                    return parent;
                };
                if child.kind() != option.item_kind() {
                    self.ctx.diagnostics.push(
                        TypeMismatch {
                            span: *child.meta(),
                            expected: RitoType::simple(option.item_kind()),
                            expected_span: None, // TODO: would be nice here
                            got: child.rito_type().into(),
                        }
                        .unwrap(),
                    );
                    return parent;
                }

                *option = values::Optional::new_with_meta(
                    option.item_kind(),
                    Some(child),
                    *option.meta(),
                )
                .unwrap();
            }
            other => {
                self.ctx.diagnostics.push(
                    UnexpectedContainerItem {
                        span: *other.meta(),
                        expected: other.rito_type(),
                        expected_span: None,
                    }
                    .unwrap(),
                );

                eprintln!("cant inject into {:?}", other.kind())
            }
        }
        parent
    }
}

fn populate_vec_or_color(
    target: &mut IrItem,
    items: &mut Vec<IrListItem>,
) -> Result<(), MaybeSpanDiag> {
    let resolve_f32 = |n: PropertyValueEnum<Span>| -> Result<f32, MaybeSpanDiag> {
        match n {
            PropertyValueEnum::F32(values::F32 { value: n, .. }) => Ok(n),
            _ => Err(TypeMismatch {
                span: *n.meta(),
                expected: RitoType::simple(PropertyKind::F32),
                expected_span: None, // TODO: would be nice
                got: RitoTypeOrVirtual::RitoType(RitoType::simple(n.kind())),
            }
            .into()),
        }
    };
    let resolve_u8 = |n: PropertyValueEnum<Span>| -> Result<u8, MaybeSpanDiag> {
        match n {
            PropertyValueEnum::U8(values::U8 { value: n, .. }) => Ok(n),
            _ => Err(TypeMismatch {
                span: *n.meta(),
                expected: RitoType::simple(PropertyKind::U8),
                expected_span: None, // TODO: would be nice
                got: RitoTypeOrVirtual::RitoType(RitoType::simple(n.kind())),
            }
            .into()),
        }
    };

    let mut items = items.drain(..);
    let mut get_next = |span: &mut Span| -> Result<_, Diagnostic> {
        let item = items
            .next()
            .ok_or(NotEnoughItems {
                span: *span,
                got: 0,
                expected: ColorOrVec::Vec2,
            })?
            .0;
        *span = *item.meta();
        Ok(item)
    };

    use PropertyValueEnum as V;
    let mut span = Span::new(0, 0); // FIXME: get a span in here stat
    let expected;
    match target.value_mut() {
        V::Vector2(values::Vector2 { value: vec, .. }) => {
            vec.x = resolve_f32(get_next(&mut span)?)?;
            vec.y = resolve_f32(get_next(&mut span)?)?;
            expected = ColorOrVec::Vec2;
        }
        V::Vector3(values::Vector3 { value: vec, .. }) => {
            vec.x = resolve_f32(get_next(&mut span)?)?;
            vec.y = resolve_f32(get_next(&mut span)?)?;
            vec.z = resolve_f32(get_next(&mut span)?)?;
            expected = ColorOrVec::Vec3;
        }
        V::Vector4(values::Vector4 { value: vec, .. }) => {
            vec.x = resolve_f32(get_next(&mut span)?)?;
            vec.y = resolve_f32(get_next(&mut span)?)?;
            vec.z = resolve_f32(get_next(&mut span)?)?;
            vec.w = resolve_f32(get_next(&mut span)?)?;
            expected = ColorOrVec::Vec4;
        }
        V::Color(values::Color { value: color, .. }) => {
            color.r = resolve_u8(get_next(&mut span)?)?;
            color.g = resolve_u8(get_next(&mut span)?)?;
            color.b = resolve_u8(get_next(&mut span)?)?;
            color.a = resolve_u8(get_next(&mut span)?)?;
            expected = ColorOrVec::Color;
        }
        V::Matrix44(values::Matrix44 { value: mat, .. }) => {
            mat.x_axis = Vec4::new(
                resolve_f32(get_next(&mut span)?)?,
                resolve_f32(get_next(&mut span)?)?,
                resolve_f32(get_next(&mut span)?)?,
                resolve_f32(get_next(&mut span)?)?,
            );
            mat.y_axis = Vec4::new(
                resolve_f32(get_next(&mut span)?)?,
                resolve_f32(get_next(&mut span)?)?,
                resolve_f32(get_next(&mut span)?)?,
                resolve_f32(get_next(&mut span)?)?,
            );
            mat.z_axis = Vec4::new(
                resolve_f32(get_next(&mut span)?)?,
                resolve_f32(get_next(&mut span)?)?,
                resolve_f32(get_next(&mut span)?)?,
                resolve_f32(get_next(&mut span)?)?,
            );
            mat.w_axis = Vec4::new(
                resolve_f32(get_next(&mut span)?)?,
                resolve_f32(get_next(&mut span)?)?,
                resolve_f32(get_next(&mut span)?)?,
                resolve_f32(get_next(&mut span)?)?,
            );
            *mat = mat.transpose();
            expected = ColorOrVec::Mat44;
        }
        _ => {
            unreachable!("non-empty list queue with non color/vec type receiver?");
        }
    }

    if let Some(extra) = items.next() {
        let count = 1 + items.count();
        return Err(TooManyItems {
            span: *extra.0.meta(),
            extra: count as _,
            expected,
        }
        .into());
    }
    Ok(())
}

impl Visitor for TypeChecker<'_> {
    fn enter_tree(&mut self, tree: &Cst) -> Visit {
        self.depth += 1;
        let depth = self.depth;

        let indent = "  ".repeat(depth.saturating_sub(1) as _);
        if std::env::var("RB_STACK").is_ok() {
            eprintln!("{indent}> d:{} | {:?}", depth, tree.kind);
            eprint!("{indent}  stack: ");
            if self.stack.is_empty() {
                eprint!("empty")
            }
            eprintln!();
            for s in &self.stack {
                eprintln!("{indent}    - {}: {:?}", s.0, s.1);
            }
        }

        let parent = self.stack.last();

        match tree.kind {
            Kind::ErrorTree => return Visit::Skip,

            Kind::ListItemBlock => {
                let Some((_, parent)) = parent else {
                    self.ctx
                        .diagnostics
                        .push(RootNonEntry.default_span(tree.span));
                    return Visit::Skip;
                };

                let parent_type = parent.value().rito_type();

                use PropertyKind as K;
                match parent_type.base {
                    K::Container | K::UnorderedContainer | K::Optional => {
                        let value_type = parent_type
                            .value_subtype()
                            .expect("container must have value_subtype");
                        self.stack.push((
                            depth,
                            IrItem::ListItem(IrListItem({
                                let mut v = value_type.default_value();
                                *v.meta_mut() = tree.span;
                                v
                            })),
                        ));
                    }
                    parent_type => {
                        eprintln!(
                            "[warn] got {parent_type:?} in ListItemBlock - {:?}",
                            &self.ctx.text[tree.span]
                        );
                    }
                }
            }
            Kind::ListItem => {
                let Some((_, parent)) = parent else {
                    self.ctx
                        .diagnostics
                        .push(RootNonEntry.default_span(tree.span));
                    return Visit::Skip;
                };

                let parent_type = parent.value().rito_type();

                use PropertyKind as K;
                let color_vec_type = match parent_type.base {
                    K::Vector2 | K::Vector3 | K::Vector4 | K::Matrix44 => Some(K::F32),
                    K::Color => Some(K::U8),
                    _ => None,
                };

                // dbg!(color_vec_type, parent_type);

                let value_hint = color_vec_type.or(parent_type.value_subtype());

                match resolve_value(&mut self.ctx, tree, value_hint) {
                    Ok(Some(item)) => {
                        // eprintln!("{indent}  list item {item:?}");
                        if color_vec_type.is_some() {
                            self.list_queue.push(IrListItem(item));
                        } else {
                            self.stack.push((depth, IrItem::ListItem(IrListItem(item))));
                        }
                    }
                    Ok(None) => {
                        // eprintln!("{indent}  ERROR empty item");
                    }
                    Err(e) => self.ctx.diagnostics.push(e.default_span(tree.span)),
                }
            }

            Kind::Entry => {
                match resolve_entry(&mut self.ctx, tree, parent.map(|p| p.1.value().rito_type()))
                    .map_err(|e| e.fallback(tree.span))
                {
                    Ok(entry) => {
                        // eprintln!("{indent}  push {entry:?}");
                        self.stack.push((depth, IrItem::Entry(entry)));
                    }
                    Err(e) => self.ctx.diagnostics.push(e),
                }
            }

            _ => {}
        }

        // match self.current.as_mut() {
        //     Some((depth, name, value)) => {}
        //     None => {
        //         match tree.kind {
        //             Kind::Entry => {}
        //             Kind::File => return Visit::Continue,
        //             kind => {
        //                 if depth == 2 {
        //                     self.ctx
        //                         .diagnostics
        //                         .push(RootNonEntry.default_span(tree.span));
        //                 }
        //                 return Visit::Skip;
        //             }
        //         }
        //
        //     }
        // }

        Visit::Continue
    }

    fn exit_tree(&mut self, tree: &cst::Cst) -> Visit {
        let depth = self.depth;
        self.depth -= 1;
        let indent = "  ".repeat(depth.saturating_sub(1) as _);
        if std::env::var("RB_STACK").is_ok() {
            eprintln!("{indent}< d:{} | {:?}", depth, tree.kind);
            eprint!("{indent}  stack: ");
            if self.stack.is_empty() {
                eprint!("empty")
            }
            eprintln!();
            for s in &self.stack {
                eprintln!("{indent}    - {}: {:?}", s.0, s.1);
            }
        }
        if tree.kind == cst::Kind::ErrorTree {
            return Visit::Continue;
        }

        match self.stack.pop() {
            Some(mut ir) => {
                if std::env::var("RB_STACK").is_ok() {
                    eprintln!("{indent}< popped {}", ir.0);
                }
                if ir.0 != depth {
                    self.stack.push(ir);
                    return Visit::Continue;
                }

                if !self.list_queue.is_empty() {
                    // let (d, mut ir) = &mut ir;
                    if let Err(e) = populate_vec_or_color(&mut ir.1, &mut self.list_queue) {
                        self.ctx.diagnostics.push(e.fallback(*ir.1.value().meta()));
                    }
                    // self.stack.push((d, ir));
                    // return Visit::Continue;
                }

                match self.stack.pop() {
                    Some((d, parent)) => {
                        let parent = self.merge_ir(parent, ir.1);
                        self.stack.push((d, parent));
                    }
                    None => {
                        if depth != 2 {
                            // eprintln!("ERROR: depth not 2??? - {depth}");
                            // eprintln!("{ir:?}");
                            return Visit::Continue;
                        }
                        // assert_eq!(depth, 2);
                        let (
                            _,
                            IrItem::Entry(IrEntry {
                                key:
                                    PropertyValueEnum::String(values::String {
                                        value: key,
                                        meta: key_span,
                                    }),
                                value,
                            }),
                        ) = ir.clone()
                        else {
                            self.ctx
                                .diagnostics
                                .push(RootNonEntry.default_span(tree.span));
                            return Visit::Continue;
                        };
                        if let Some(existing) = self.root.insert(key, value) {
                            self.ctx.diagnostics.push(
                                ShadowedEntry {
                                    shadowee: *existing.meta(),
                                    shadower: key_span,
                                }
                                .unwrap(),
                            );
                        }
                    }
                }
                // TODO: warn when shadowed
            }
            _ => {
                // eprintln!("exit tree with no current?");
            }
        }

        Visit::Continue
    }
}
