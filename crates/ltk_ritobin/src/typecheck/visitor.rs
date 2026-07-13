use std::{
    borrow::Cow,
    fmt::{Debug, Display},
    vec::Drain,
};

use glam::Vec4;
use indexmap::{Equivalent, IndexMap};
use ltk_hash::{BinHash, Hash as _, WadHash};
use ltk_meta::{
    property::values, traits::PropertyExt, Bin, BinObject, PropertyKind, PropertyValueEnum,
};

use crate::{
    cst::{
        self,
        visitor::{Visit, VisitCtx},
        Kind, Node, NodeId, Visitor,
    },
    parse::{Span, Token, TokenKind},
    typecheck::{
        diagnostics::{self, Diagnostic, DiagnosticWithSpan, MaybeSpanDiag},
        ir::{IrEntry, IrItem, IrListItem},
    },
    Cst, PropertyValueExt as _, RitoType, RitobinName,
};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum RootKind {
    Type,
    Version,
    Linked,
    Entries,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootKindOrUnknown<'a> {
    Known(RootKind),
    Unknown(Cow<'a, str>),
}

impl std::hash::Hash for RootKindOrUnknown<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            RootKindOrUnknown::Known(root_kind) => root_kind.hash(state),
            RootKindOrUnknown::Unknown(cow) => cow.hash(state),
        }
    }
}

impl Equivalent<RootKindOrUnknown<'_>> for RootKind {
    #[inline(always)]
    fn equivalent(&self, key: &RootKindOrUnknown<'_>) -> bool {
        match key {
            RootKindOrUnknown::Known(root_kind) => self == root_kind,
            RootKindOrUnknown::Unknown(_) => false,
        }
    }
}
impl Equivalent<RootKindOrUnknown<'_>> for Cow<'_, str> {
    #[inline(always)]
    fn equivalent(&self, key: &RootKindOrUnknown<'_>) -> bool {
        match key {
            RootKindOrUnknown::Known(_) => false,
            RootKindOrUnknown::Unknown(cow) => self == cow,
        }
    }
}
impl Equivalent<RootKindOrUnknown<'_>> for str {
    #[inline(always)]
    fn equivalent(&self, key: &RootKindOrUnknown<'_>) -> bool {
        match key {
            RootKindOrUnknown::Known(_) => false,
            RootKindOrUnknown::Unknown(cow) => self == cow,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::typecheck::visitor::{RootKind, RootKindOrUnknown};

    #[test]
    fn root_kind_eq() {
        let mut root: IndexMap<RootKindOrUnknown<'static>, ()> = Default::default();

        root.insert(RootKind::Version.into(), ());
        root.insert(RootKind::Entries.into(), ());
        root.insert(RootKindOrUnknown::Unknown("foo".into()), ());
        root.insert(RootKindOrUnknown::Unknown("bar".into()), ());

        assert!(root.swap_remove(&RootKind::Version).is_some());
        assert!(root.swap_remove(&RootKind::Entries).is_some());
        assert!(root
            .swap_remove(&RootKindOrUnknown::Unknown("bar".into()))
            .is_some());

        assert_eq!(root.len(), 1);
    }
}

impl<'a> RootKindOrUnknown<'a> {
    pub fn from_value(src: &'a str, value: &PropertyValueEnum<Span>) -> Self {
        let PropertyValueEnum::String(string) = value else {
            return Self::Unknown(src[*value.meta()].into());
        };

        match string.as_str() {
            "type" => RootKind::Type.into(),
            "version" => RootKind::Version.into(),
            "linked" => RootKind::Linked.into(),
            "entries" => RootKind::Entries.into(),
            _ => Self::Unknown(src[*value.meta()].into()),
        }
    }
}

impl From<RootKind> for RootKindOrUnknown<'_> {
    #[inline(always)]
    fn from(value: RootKind) -> Self {
        Self::Known(value)
    }
}
impl<'a> From<Cow<'a, str>> for RootKindOrUnknown<'a> {
    #[inline(always)]
    fn from(value: Cow<'a, str>) -> Self {
        Self::Unknown(value)
    }
}

#[derive(Debug, Clone)]
pub struct RootEntry {
    key: PropertyValueEnum<Span>,
    type_span: Span,
    value: PropertyValueEnum<Span>,
}

pub struct TypeChecker<'a> {
    ctx: Ctx<'a>,
    pub root: IndexMap<RootKindOrUnknown<'a>, RootEntry>,
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
}

#[derive(Debug, Clone, Copy)]
pub enum RitoTypeOrVirtual {
    RitoType(RitoType),
    Numeric,
    StructOrEmbedded,
    Token(TokenKind),
    Tree(Kind),
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
            Self::Token(kind) => Display::fmt(kind, f),
            Self::Tree(kind) => Display::fmt(kind, f),
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

use diagnostics::Diagnostic::*;

trait TreeIterExt<'a>: Iterator {
    fn expect_tree(&mut self, cst: &'a Cst, kind: cst::Kind) -> Result<&'a Node, Diagnostic>;
    fn expect_token(&mut self, cst: &'a Cst, kind: TokenKind) -> Result<&'a Token, Diagnostic>;
}

impl<'a, I> TreeIterExt<'a> for I
where
    I: Iterator<Item = &'a cst::Child>,
{
    fn expect_tree(&mut self, cst: &'a Cst, kind: cst::Kind) -> Result<&'a Node, Diagnostic> {
        self.find_map(|c| c.tree(cst).filter(|t| t.kind == kind))
            .ok_or(MissingTree(kind))
    }
    fn expect_token(&mut self, cst: &'a Cst, kind: TokenKind) -> Result<&'a Token, Diagnostic> {
        self.find_map(|c| c.token(cst).filter(|t| t.kind == kind))
            .ok_or(MissingToken(kind))
    }
}

pub struct Ctx<'a> {
    text: &'a str,
    diagnostics: Vec<DiagnosticWithSpan>,
}

pub fn coerce_type<M: Debug + Default>(
    value: PropertyValueEnum<M>,
    to: PropertyKind,
) -> Option<PropertyValueEnum<M>> {
    match to {
        to if to == value.kind() => Some(value),

        PropertyKind::Optional => Some(values::Optional::try_from(value).ok()?.into()),

        PropertyKind::Hash => match value {
            PropertyValueEnum::String(str) => {
                Some(values::Hash::new_with_meta(BinHash::hash_str(&str), str.meta).into())
            }
            _ => None,
        },
        PropertyKind::ObjectLink => match value {
            PropertyValueEnum::Hash(hash) => {
                Some(values::ObjectLink::new_with_meta(*hash, hash.meta).into())
            }
            PropertyValueEnum::String(str) => {
                Some(values::ObjectLink::new_with_meta(BinHash::hash_str(&str), str.meta).into())
            }
            _ => None,
        },
        PropertyKind::WadChunkLink => match value {
            PropertyValueEnum::Hash(hash) => Some(
                values::WadChunkLink::new_with_meta(WadHash((**hash).into()), hash.meta).into(),
            ),
            PropertyValueEnum::String(str) => Some(
                values::WadChunkLink::new_with_meta(WadHash::hash_str(str.as_str()), str.meta)
                    .into(),
            ),
            _ => None,
        },
        PropertyKind::BitBool => match value {
            PropertyValueEnum::Bool(bool) => {
                Some(values::BitBool::new_with_meta(*bool, bool.meta).into())
            }
            _ => None,
        },
        PropertyKind::Bool => match value {
            PropertyValueEnum::BitBool(bool) => {
                Some(values::Bool::new_with_meta(*bool, bool.meta).into())
            }
            _ => None,
        },
        _ => None,
    }
}

pub fn resolve_rito_type(
    ctx: &mut Ctx<'_>,
    visit_ctx: &VisitCtx,
    tree: &Node,
) -> Result<RitoType, Diagnostic> {
    let mut c = tree.children.get(visit_ctx.cst).iter();

    let base = c.expect_token(visit_ctx.cst, TokenKind::Name)?;
    let base_span = base.span;

    let base = PropertyKind::from_rito_name(&ctx.text[base.span]).ok_or(UnknownType(base.span))?;

    let subtypes = match c.clone().find_map(|c| {
        c.tree(visit_ctx.cst)
            .filter(|t| t.kind == Kind::TypeArgList)
    }) {
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
                .get(visit_ctx.cst)
                .iter()
                .filter_map(|c| c.tree(visit_ctx.cst).filter(|t| t.kind == Kind::TypeArg))
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

    // since we can't know whether bin/wad was intended, we will just try fit it in the smallest hash that allows it.
    // we can then safely coerce the type upwards when we are given type information
    Ok(match BinHash::from_str_radix(src, 16) {
        Ok(hash) => PropertyValueEnum::Hash(values::Hash::new_with_meta(hash, span)),
        Err(_) => match WadHash::from_str_radix(src, 16) {
            Ok(hash) => {
                PropertyValueEnum::WadChunkLink(values::WadChunkLink::new_with_meta(hash, span))
            }
            Err(_) => return Err(InvalidHash(span)),
        },
    })
}

pub fn resolve_literal(
    ctx: &mut Ctx,
    token: &Token,
    kind_hint: Option<RitoType>,
) -> Result<Option<PropertyValueEnum<Span>>, Diagnostic> {
    use PropertyKind as K;
    use PropertyValueEnum as P;
    Ok(Some(match token {
        Token {
            kind: TokenKind::String,
            span,
        } => values::String::new_with_meta(
            ctx.text[Span::new(span.start + 1, span.end - 1)].into(),
            *span,
        )
        .into(),

        Token {
            kind: TokenKind::True,
            span,
        } => values::Bool::new_with_meta(true, *span).into(),
        Token {
            kind: TokenKind::False,
            span,
        } => values::Bool::new_with_meta(false, *span).into(),

        Token {
            kind: TokenKind::HexLit,
            span,
        } => resolve_hash(ctx, *span)?,
        Token {
            kind: TokenKind::Number,
            span,
        } => {
            let txt = &ctx.text[span];
            let Some(kind_hint) = kind_hint else {
                return Err(AmbiguousNumeric(*span));
            };

            let txt = match txt.contains('_') {
                true => Cow::Owned(txt.replace('_', "")),
                false => Cow::Borrowed(txt),
            };

            let kind_hint = match kind_hint.base {
                K::Optional => kind_hint.value_subtype().unwrap(),
                base => base,
            };

            match kind_hint {
                K::U8 => P::U8(values::U8::new_with_meta(
                    txt.parse::<u8>()
                        .map_err(|e| Diagnostic::ParseNumericError {
                            expected: kind_hint,
                            error: Some(*e.kind()),
                            span: *span,
                        })?,
                    *span,
                )),
                K::U16 => P::U16(values::U16::new_with_meta(
                    txt.parse::<u16>()
                        .map_err(|e| Diagnostic::ParseNumericError {
                            expected: kind_hint,
                            error: Some(*e.kind()),
                            span: *span,
                        })?,
                    *span,
                )),
                K::U32 => P::U32(values::U32::new_with_meta(
                    txt.parse::<u32>()
                        .map_err(|e| Diagnostic::ParseNumericError {
                            expected: kind_hint,
                            error: Some(*e.kind()),
                            span: *span,
                        })?,
                    *span,
                )),
                K::U64 => P::U64(values::U64::new_with_meta(
                    txt.parse::<u64>()
                        .map_err(|e| Diagnostic::ParseNumericError {
                            expected: kind_hint,
                            error: Some(*e.kind()),
                            span: *span,
                        })?,
                    *span,
                )),
                K::I8 => P::I8(values::I8::new_with_meta(
                    txt.parse::<i8>()
                        .map_err(|e| Diagnostic::ParseNumericError {
                            expected: kind_hint,
                            error: Some(*e.kind()),
                            span: *span,
                        })?,
                    *span,
                )),
                K::I16 => P::I16(values::I16::new_with_meta(
                    txt.parse::<i16>()
                        .map_err(|e| Diagnostic::ParseNumericError {
                            expected: kind_hint,
                            error: Some(*e.kind()),
                            span: *span,
                        })?,
                    *span,
                )),
                K::I32 => P::I32(values::I32::new_with_meta(
                    txt.parse::<i32>()
                        .map_err(|e| Diagnostic::ParseNumericError {
                            expected: kind_hint,
                            error: Some(*e.kind()),
                            span: *span,
                        })?,
                    *span,
                )),
                K::I64 => P::I64(values::I64::new_with_meta(
                    txt.parse::<i64>()
                        .map_err(|e| Diagnostic::ParseNumericError {
                            expected: kind_hint,
                            error: Some(*e.kind()),
                            span: *span,
                        })?,
                    *span,
                )),
                K::F32 => P::F32(values::F32::new_with_meta(
                    txt.parse().map_err(|_| Diagnostic::ParseNumericError {
                        expected: kind_hint,
                        error: None,
                        span: *span,
                    })?,
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
    }))
}

pub fn resolve_value(
    ctx: &mut Ctx,
    visit_ctx: &VisitCtx,
    tree: &Node,
    kind_hint: Option<RitoType>,
) -> Result<Option<PropertyValueEnum<Span>>, Diagnostic> {
    use PropertyKind as K;
    use PropertyValueEnum as P;

    let Some(child) = tree.children.get(visit_ctx.cst).first() else {
        return Ok(None);
    };
    Ok(Some(match child.tree(visit_ctx.cst) {
        Some(Node {
            kind: Kind::Class,
            children,
            span,
            ..
        }) => {
            let Some(kind_hint) = kind_hint else {
                return Ok(None); // TODO: err
            };
            let Some(class) = children
                .get(visit_ctx.cst)
                .first()
                .and_then(|t| t.token(visit_ctx.cst))
            else {
                return Err(InvalidHash(*span));
            };

            let class_hash = match class {
                Token {
                    kind: TokenKind::Name,
                    span,
                } => BinHash::hash_str(&ctx.text[span]),
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
            match kind_hint.base {
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
                    #[cfg(feature = "debug")]
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
        Some(Node {
            kind: Kind::Literal,
            children,
            ..
        }) => {
            let Some(child) = children.get(visit_ctx.cst).first() else {
                return Ok(None);
            };
            return resolve_literal(ctx, child.token(visit_ctx.cst).unwrap(), kind_hint);
        }
        _ => return Ok(None),
    }))
}

pub fn resolve_entry(
    ctx: &mut Ctx,
    visit_ctx: &VisitCtx,
    tree: &Node,
    parent_value_kind: Option<RitoType>,
) -> Result<IrEntry, MaybeSpanDiag> {
    let mut c = tree.children.get(visit_ctx.cst).iter();

    let key = c.expect_tree(visit_ctx.cst, Kind::EntryKey)?;

    let key = match key
        .children
        .get(visit_ctx.cst)
        .first()
        .ok_or(InvalidHash(key.span))?
        .token(visit_ctx.cst)
    {
        Some(Token {
            kind: TokenKind::Name,
            span,
        }) => PropertyValueEnum::from(values::String::new_with_meta(ctx.text[span].into(), *span)),
        Some(Token {
            kind: TokenKind::String,
            span,
        }) => PropertyValueEnum::from(values::String::new_with_meta(
            ctx.text[Span::new(span.start + 1, span.end - 1)].into(),
            *span,
        )),
        Some(Token {
            kind: TokenKind::HexLit,
            span,
        }) => resolve_hash(ctx, *span)?,
        Some(token) => resolve_literal(
            ctx,
            token,
            parent_value_kind
                .and_then(|k| k.subtypes[0])
                .map(RitoType::simple),
        )?
        .ok_or(CustomSpan("erm idk bad literal", key.span))?,
        _ => {
            return Err(InvalidHash(key.span).into());
        }
    };

    let parent_value_kind = parent_value_kind
        .and_then(|p| p.value_subtype())
        .map(RitoType::simple);

    let kind = c
        .clone()
        .find_map(|c| c.tree(visit_ctx.cst).filter(|t| t.kind == Kind::TypeExpr));
    let kind_span = kind.map(|k| k.span);
    let kind = kind
        .map(|t| resolve_rito_type(ctx, visit_ctx, t))
        .transpose()?;

    let value = c.expect_tree(visit_ctx.cst, Kind::EntryValue)?;
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

    let resolved_val = match resolve_value(ctx, visit_ctx, value, kind) {
        Ok(v) => v,
        Err(e) => Some(match kind {
            Some(kind) => {
                ctx.diagnostics.push(e.default_span(tree.span));
                kind.make_default(value.span)
            }
            None => {
                return Err(e.into());
            }
        }),
    };

    let resolved_val = resolved_val.map(|value| match kind {
        Some(kind) if value.kind() == kind.base => value,
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
        let dependencies = self
            .root
            .swap_remove(&RootKindOrUnknown::Known(RootKind::Linked));

        if dependencies.is_none() {
            self.ctx.diagnostics.push(
                MissingRootEntry {
                    root_kind: RootKind::Linked,
                }
                .unwrap(),
            );
        }

        let dependencies = dependencies.and_then(|e| {
            let PropertyValueEnum::Container(list) = e.value else {
                self.ctx.diagnostics.push(
                    InvalidRootEntryType {
                        root_kind: RootKind::Linked,

                        key_span: *e.key.meta(),
                        type_span: e.type_span,

                        expected: RitoType::simple(PropertyKind::Container),
                        got: RitoType::simple(e.value.kind()),
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
            .swap_remove(&RootKindOrUnknown::Known(RootKind::Entries));

        if objects.is_none() {
            self.ctx.diagnostics.push(
                MissingRootEntry {
                    root_kind: RootKind::Entries,
                }
                .unwrap(),
            );
        }

        let objects = objects.and_then(|e| {
            let PropertyValueEnum::Map(map) = e.value else {
                self.ctx.diagnostics.push(
                    InvalidRootEntryType {
                        root_kind: RootKind::Entries,
                        key_span: *e.key.meta(),
                        type_span: *e.key.meta(),
                        got: RitoType::simple(e.value.kind()),
                        expected: RitoType::simple(PropertyKind::Map),
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

                        if let PropertyValueEnum::Embedded(values::Embedded(struct_val)) = value {
                            let struct_val = struct_val.no_meta();
                            Some(BinObject {
                                path_hash: *path_hash,
                                class_hash: struct_val.class_hash,
                                properties: struct_val.properties,
                            })
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        });

        match self.root.swap_remove(&RootKind::Type) {
            Some(bin_type) => {
                if let PropertyValueEnum::String(type_value) = bin_type.value {
                    match type_value.as_str() {
                        "PROP" => {}
                        "PTCH" => {
                            self.ctx.diagnostics.push(
                                CustomSpan("Patch bins are not supported yet", *type_value.meta())
                                    .unwrap(),
                            );
                        }
                        _other => {
                            self.ctx
                                .diagnostics
                                .push(CustomSpan("Unknown bin type", *type_value.meta()).unwrap());
                        }
                    }
                } else {
                    self.ctx.diagnostics.push(
                        InvalidRootEntryType {
                            root_kind: RootKind::Version,
                            key_span: *bin_type.key.meta(),
                            type_span: *bin_type.key.meta(),
                            got: RitoType::simple(bin_type.value.kind()),
                            expected: RitoType::simple(PropertyKind::String),
                        }
                        .unwrap(),
                    );
                }
            }
            None => {
                self.ctx.diagnostics.push(
                    MissingRootEntry {
                        root_kind: RootKind::Version,
                    }
                    .default_span(Span::default()),
                );
            }
        }
        match self.root.swap_remove(&RootKind::Version) {
            Some(version) => {
                if let PropertyValueEnum::U32(version) = version.value {
                    match *version {
                        3 => {}
                        _other => {
                            self.ctx.diagnostics.push(
                                CustomSpan("Bin version should be '3'", *version.meta()).unwrap(),
                            );
                        }
                    }
                } else {
                    self.ctx.diagnostics.push(
                        InvalidRootEntryType {
                            root_kind: RootKind::Version,
                            key_span: *version.key.meta(),
                            type_span: *version.key.meta(),
                            got: RitoType::simple(version.value.kind()),
                            expected: RitoType::simple(PropertyKind::U32),
                        }
                        .unwrap(),
                    );
                }
            }
            None => {
                self.ctx.diagnostics.push(
                    MissingRootEntry {
                        root_kind: RootKind::Version,
                    }
                    .default_span(Span::default()),
                );
            }
        }

        for (_key, unknown) in self.root {
            self.ctx.diagnostics.push(
                UnknownRoot {
                    span: *unknown.key.meta(),
                }
                .default_span(Span::default()),
            );
        }

        let tree = Bin::new(
            objects.unwrap_or_default(),
            dependencies.unwrap_or_default(),
        );

        (tree, self.ctx.diagnostics)
    }

    fn merge_ir(&mut self, mut parent: IrItem, child: IrItem) -> IrItem {
        match &mut parent.value_mut() {
            PropertyValueEnum::Container(list)
            | PropertyValueEnum::UnorderedContainer(values::UnorderedContainer(list)) => {
                match child {
                    IrItem::ListItem(IrListItem(mut value)) => {
                        if value.kind() != list.item_kind() {
                            value = coerce_type(value.clone(), list.item_kind()).unwrap_or(value);
                        }

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
                        #[cfg(feature = "debug")]
                        eprintln!("\x1b[41mlist item must be list item\x1b[0m");
                        return parent;
                    }
                }
            }
            PropertyValueEnum::Struct(struct_val)
            | PropertyValueEnum::Embedded(values::Embedded(struct_val)) => {
                let IrItem::Entry(IrEntry { key, value }) = child else {
                    #[cfg(feature = "debug")]
                    eprintln!("\x1b[41mstruct item must be entry\x1b[0m");
                    return parent;
                };

                let Some(PropertyValueEnum::Hash(key)) = coerce_type(key, PropertyKind::Hash)
                else {
                    return parent;
                };

                struct_val.properties.insert(*key, value);
            }
            PropertyValueEnum::Map(map_value) => {
                let IrItem::Entry(IrEntry { key, value }) = child else {
                    #[cfg(feature = "debug")]
                    eprintln!("map item must be entry");
                    return parent;
                };
                let span = *value.meta();
                let Some(key) = coerce_type(key, map_value.key_kind()) else {
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
                    #[cfg(feature = "debug")]
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

                #[cfg(feature = "debug")]
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
    let get_next = |span: &mut Span, items: &mut Drain<'_, IrListItem>| -> Result<_, Diagnostic> {
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
    let mut span = *target.value().meta(); // TODO: is this the right span to start with?

    let inject_vec2 = |v: &mut values::Vector2<Span>,
                       span: &mut Span,
                       items: &mut Drain<'_, IrListItem>|
     -> Result<ColorOrVec, MaybeSpanDiag> {
        let values::Vector2 { value: vec, .. } = v;
        vec.x = resolve_f32(get_next(span, items)?)?;
        vec.y = resolve_f32(get_next(span, items)?)?;
        Ok(ColorOrVec::Vec2)
    };
    let inject_vec3 = |v: &mut values::Vector3<Span>,
                       span: &mut Span,
                       items: &mut Drain<'_, IrListItem>|
     -> Result<ColorOrVec, MaybeSpanDiag> {
        let values::Vector3 { value: vec, .. } = v;
        vec.x = resolve_f32(get_next(span, items)?)?;
        vec.y = resolve_f32(get_next(span, items)?)?;
        vec.z = resolve_f32(get_next(span, items)?)?;
        Ok(ColorOrVec::Vec3)
    };
    let inject_vec4 = |v: &mut values::Vector4<Span>,
                       span: &mut Span,
                       items: &mut Drain<'_, IrListItem>|
     -> Result<ColorOrVec, MaybeSpanDiag> {
        let values::Vector4 { value: vec, .. } = v;
        vec.x = resolve_f32(get_next(span, items)?)?;
        vec.y = resolve_f32(get_next(span, items)?)?;
        vec.z = resolve_f32(get_next(span, items)?)?;
        vec.w = resolve_f32(get_next(span, items)?)?;
        Ok(ColorOrVec::Vec4)
    };
    let inject_color = |v: &mut values::Color<Span>,
                        span: &mut Span,
                        items: &mut Drain<'_, IrListItem>|
     -> Result<ColorOrVec, MaybeSpanDiag> {
        let values::Color { value: color, .. } = v;
        color.r = resolve_u8(get_next(span, items)?)?;
        color.g = resolve_u8(get_next(span, items)?)?;
        color.b = resolve_u8(get_next(span, items)?)?;
        color.a = resolve_u8(get_next(span, items)?)?;
        Ok(ColorOrVec::Color)
    };
    let inject_mat44 = |v: &mut values::Matrix44<Span>,
                        span: &mut Span,
                        items: &mut Drain<'_, IrListItem>|
     -> Result<ColorOrVec, MaybeSpanDiag> {
        let values::Matrix44 { value: mat, .. } = v;
        mat.x_axis = Vec4::new(
            resolve_f32(get_next(span, items)?)?,
            resolve_f32(get_next(span, items)?)?,
            resolve_f32(get_next(span, items)?)?,
            resolve_f32(get_next(span, items)?)?,
        );
        mat.y_axis = Vec4::new(
            resolve_f32(get_next(span, items)?)?,
            resolve_f32(get_next(span, items)?)?,
            resolve_f32(get_next(span, items)?)?,
            resolve_f32(get_next(span, items)?)?,
        );
        mat.z_axis = Vec4::new(
            resolve_f32(get_next(span, items)?)?,
            resolve_f32(get_next(span, items)?)?,
            resolve_f32(get_next(span, items)?)?,
            resolve_f32(get_next(span, items)?)?,
        );
        mat.w_axis = Vec4::new(
            resolve_f32(get_next(span, items)?)?,
            resolve_f32(get_next(span, items)?)?,
            resolve_f32(get_next(span, items)?)?,
            resolve_f32(get_next(span, items)?)?,
        );
        *mat = mat.transpose();
        Ok(ColorOrVec::Mat44)
    };

    let mut inject =
        |target: &mut PropertyValueEnum<Span>| -> Result<Option<ColorOrVec>, MaybeSpanDiag> {
            Ok(Some(match target {
                V::Vector2(v) => inject_vec2(v, &mut span, &mut items)?,
                V::Vector3(v) => inject_vec3(v, &mut span, &mut items)?,
                V::Vector4(v) => inject_vec4(v, &mut span, &mut items)?,
                V::Color(v) => inject_color(v, &mut span, &mut items)?,
                V::Matrix44(v) => inject_mat44(v, &mut span, &mut items)?,
                V::Optional(opt) => match opt {
                    values::Optional::Vector2 { value, .. } => {
                        inject_vec2(value.get_or_insert_default(), &mut span, &mut items)?
                    }
                    values::Optional::Vector3 { value, .. } => {
                        inject_vec3(value.get_or_insert_default(), &mut span, &mut items)?
                    }
                    values::Optional::Vector4 { value, .. } => {
                        inject_vec4(value.get_or_insert_default(), &mut span, &mut items)?
                    }
                    values::Optional::Color { value, .. } => {
                        inject_color(value.get_or_insert_default(), &mut span, &mut items)?
                    }
                    values::Optional::Matrix44 { value, .. } => {
                        inject_mat44(value.get_or_insert_default(), &mut span, &mut items)?
                    }
                    _ => return Ok(None),
                },
                _ => return Ok(None),
            }))
        };

    let expected = inject(target.value_mut())?.ok_or(CustomSpan(
        "non-empty list queue with non color/vec type receiver",
        span,
    ))?;

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
    fn enter_tree(&mut self, ctx: &VisitCtx, tree: NodeId) -> Visit {
        let tree = ctx.node(tree).unwrap();
        self.depth += 1;
        let depth = self.depth;

        #[cfg(feature = "debug")]
        let indent = "  ".repeat(depth.saturating_sub(1) as _);

        #[cfg(feature = "debug")]
        {
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
                    _parent_type => {
                        self.ctx.diagnostics.push(
                            UnexpectedTree {
                                tree: tree.kind,
                                expected: Some(Kind::Entry),
                                span: tree.span,
                            }
                            .unwrap(),
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

                let get_color_vec_type = |kind: PropertyKind| match kind {
                    K::Vector2 | K::Vector3 | K::Vector4 | K::Matrix44 => Some(K::F32),
                    K::Color => Some(K::U8),
                    _ => None,
                };

                let color_vec_type = get_color_vec_type(parent_type.base)
                    .or(parent_type.value_subtype().and_then(get_color_vec_type));

                let value_hint = color_vec_type
                    .or(parent_type.value_subtype())
                    .map(RitoType::simple);

                match resolve_value(&mut self.ctx, ctx, tree, value_hint) {
                    Ok(Some(item)) => {
                        #[cfg(feature = "debug")]
                        eprintln!("{indent}  list item {item:?}");
                        if color_vec_type.is_some() {
                            self.list_queue.push(IrListItem(item));
                        } else {
                            self.stack.push((depth, IrItem::ListItem(IrListItem(item))));
                        }
                    }
                    Ok(None) => {
                        #[cfg(feature = "debug")]
                        eprintln!("{indent}  ERROR empty item");
                        for child in tree.children.get(ctx.cst).iter() {
                            let (got, span) = match child {
                                cst::Child::Token(token_id) => {
                                    let tok = ctx.cst.token(*token_id).unwrap();
                                    (RitoTypeOrVirtual::Token(tok.kind), tok.span)
                                }
                                cst::Child::Tree(node_id) => {
                                    let node = ctx.cst.node(*node_id).unwrap();
                                    (RitoTypeOrVirtual::Tree(node.kind), node.span)
                                }
                            };
                            self.ctx.diagnostics.push(
                                TypeMismatch {
                                    span,
                                    got,
                                    expected: value_hint
                                        .unwrap_or(RitoType::simple(PropertyKind::None)),
                                    expected_span: None,
                                }
                                .unwrap(),
                            );
                        }
                    }
                    Err(e) => self.ctx.diagnostics.push(e.default_span(tree.span)),
                }
            }

            Kind::Entry => {
                match resolve_entry(
                    &mut self.ctx,
                    ctx,
                    tree,
                    parent.map(|p| p.1.value().rito_type()),
                )
                .map_err(|e| e.fallback(tree.span))
                {
                    Ok(entry) => {
                        self.stack.push((depth, IrItem::Entry(entry)));
                    }
                    Err(e) => self.ctx.diagnostics.push(e),
                }
            }

            _ => {}
        }

        match self.stack.last() {
            Some(_) => {}
            None => match tree.kind {
                Kind::Entry | Kind::Comment | Kind::File => return Visit::Continue,
                _ => {
                    if depth == 2 {
                        self.ctx
                            .diagnostics
                            .push(RootNonEntry.default_span(tree.span));
                    }
                    return Visit::Skip;
                }
            },
        }

        Visit::Continue
    }

    fn exit_tree(&mut self, ctx: &VisitCtx, tree: NodeId) -> Visit {
        let tree = ctx.node(tree).unwrap();
        let depth = self.depth;
        self.depth -= 1;

        #[cfg(feature = "debug")]
        let indent = "  ".repeat(depth.saturating_sub(1) as _);
        #[cfg(feature = "debug")]
        {
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
        }
        if tree.kind == cst::Kind::ErrorTree {
            return Visit::Continue;
        }

        match self.stack.pop() {
            Some(mut ir) => {
                #[cfg(feature = "debug")]
                {
                    if std::env::var("RB_STACK").is_ok() {
                        eprintln!("{indent}< popped {}", ir.0);
                    }
                }
                if ir.0 != depth {
                    self.stack.push(ir);
                    return Visit::Continue;
                }

                if !self.list_queue.is_empty() {
                    if let Err(e) = populate_vec_or_color(&mut ir.1, &mut self.list_queue) {
                        self.ctx.diagnostics.push(e.fallback(*ir.1.value().meta()));
                    }
                }

                match self.stack.pop() {
                    Some((d, parent)) => {
                        let parent = self.merge_ir(parent, ir.1);
                        self.stack.push((d, parent));
                    }
                    None => {
                        if depth != 2 {
                            return Visit::Continue;
                        }
                        let IrItem::Entry(IrEntry {
                            key: key @ PropertyValueEnum::String(values::String { .. }),
                            value,
                        }) = ir.1
                        else {
                            self.ctx
                                .diagnostics
                                .push(RootNonEntry.default_span(tree.span));
                            return Visit::Continue;
                        };
                        let key_span = *key.meta();
                        if let Some(existing) = self.root.insert(
                            RootKindOrUnknown::from_value(self.ctx.text, &key),
                            RootEntry {
                                key,
                                type_span: key_span,
                                value,
                            }, // FIXME: get real type span in here
                        ) {
                            self.ctx.diagnostics.push(
                                ShadowedEntry {
                                    shadowee: *existing.key.meta(),
                                    shadower: key_span,
                                }
                                .unwrap(),
                            );
                        }
                    }
                }
            }
            _ => {}
        }

        Visit::Continue
    }
}
