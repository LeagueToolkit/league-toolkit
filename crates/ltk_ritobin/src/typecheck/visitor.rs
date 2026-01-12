use std::{
    num::ParseIntError,
    ops::{Deref, DerefMut},
};

use indexmap::IndexMap;
use ltk_meta::{
    value::{
        ContainerValue, MapValue, NoneValue, OptionalValue, StringValue, UnorderedContainerValue,
    },
    BinPropertyKind, PropertyValueEnum,
};

use crate::{
    parse::{
        cst::{self, visitor::Visit, Cst, Kind, Visitor},
        Span, Token, TokenKind,
    },
    typecheck::RitobinName,
};

pub trait SpannedExt {
    fn with_span(self, span: Span) -> Spanned<Self>
    where
        Self: Sized,
    {
        Spanned::new(self, span)
    }
}
impl<T: Sized> SpannedExt for T {}

#[derive(Debug, Clone, Copy)]
pub struct Spanned<T> {
    pub span: Span,
    pub inner: T,
}

impl<T> Spanned<T> {
    pub fn new(item: T, span: Span) -> Self {
        Self { inner: item, span }
    }
}

impl<T> Deref for Spanned<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<T> DerefMut for Spanned<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[derive(Debug, Clone)]
pub struct IrEntry {
    pub key: Spanned<PropertyValueEnum>,
    pub value: Spanned<PropertyValueEnum>,
}

#[derive(Debug, Clone)]
pub struct IrListItem(pub Spanned<PropertyValueEnum>);

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
    pub fn value(&self) -> &Spanned<PropertyValueEnum> {
        match self {
            IrItem::Entry(i) => &i.value,
            IrItem::ListItem(i) => &i.0,
        }
    }
    pub fn value_mut(&mut self) -> &mut Spanned<PropertyValueEnum> {
        match self {
            IrItem::Entry(i) => &mut i.value,
            IrItem::ListItem(i) => &mut i.0,
        }
    }
    pub fn into_value(self) -> Spanned<PropertyValueEnum> {
        match self {
            IrItem::Entry(i) => i.value,
            IrItem::ListItem(i) => i.0,
        }
    }
}

pub struct TypeChecker<'a> {
    ctx: Ctx<'a>,
    pub root: IndexMap<String, Spanned<PropertyValueEnum>>,
    // current: Option<(PropertyValueEnum, PropertyValueEnum)>,
    stack: Vec<(u32, IrItem)>,
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
            depth: 0,
        }
    }
    pub fn into_parts(
        self,
    ) -> (
        IndexMap<String, Spanned<PropertyValueEnum>>,
        Vec<DiagnosticWithSpan>,
    ) {
        (self.root, self.ctx.diagnostics)
    }
}
#[derive(Debug, Clone, Copy)]
pub enum Diagnostic {
    MissingTree(cst::Kind),
    EmptyTree(cst::Kind),
    MissingToken(TokenKind),
    UnknownType(Span),
    MissingType(Span),

    ResolveLiteral,

    RootNonEntry,
    ShadowedEntry {
        shadowee: Span,
        shadower: Span,
    },

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
            MissingTree(_) | EmptyTree(_) | MissingToken(_) | RootNonEntry | ResolveLiteral => None,
            UnknownType(span)
            | SubtypeCountMismatch { span, .. }
            | UnexpectedSubtypes { span, .. }
            | MissingType(span)
            | ShadowedEntry { shadower: span, .. } => Some(span),
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
impl PropertyValueExt for PropertyValueEnum {
    fn rito_type(&self) -> RitoType {
        let base = self.kind();
        let subtypes = match self {
            PropertyValueEnum::Map(MapValue {
                key_kind,
                value_kind,
                ..
            }) => [Some(*key_kind), Some(*value_kind)],
            PropertyValueEnum::UnorderedContainer(UnorderedContainerValue(ContainerValue {
                item_kind,
                ..
            })) => [Some(*item_kind), None],
            PropertyValueEnum::Container(ContainerValue { item_kind, .. }) => {
                [Some(*item_kind), None]
            }
            PropertyValueEnum::Optional(OptionalValue { kind, .. }) => [Some(*kind), None],

            _ => [None, None],
        };
        RitoType { base, subtypes }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RitoType {
    pub base: BinPropertyKind,
    pub subtypes: [Option<BinPropertyKind>; 2],
}

impl RitoType {
    pub fn simple(kind: BinPropertyKind) -> Self {
        Self {
            base: kind,
            subtypes: [None, None],
        }
    }

    fn subtype(&self, idx: usize) -> BinPropertyKind {
        self.subtypes[idx].unwrap_or_default()
    }

    fn value_subtype(&self) -> Option<BinPropertyKind> {
        self.subtypes[1].or(self.subtypes[0])
    }

    pub fn make_default(&self) -> PropertyValueEnum {
        match self.base {
            BinPropertyKind::Map => PropertyValueEnum::Map(MapValue {
                key_kind: self.subtype(0),
                value_kind: self.subtype(1),
                ..Default::default()
            }),
            BinPropertyKind::UnorderedContainer => {
                PropertyValueEnum::UnorderedContainer(UnorderedContainerValue(ContainerValue {
                    item_kind: self.subtype(0),
                    ..Default::default()
                }))
            }
            BinPropertyKind::Container => PropertyValueEnum::Container(ContainerValue {
                item_kind: self.subtype(0),
                ..Default::default()
            }),
            BinPropertyKind::Optional => PropertyValueEnum::Optional(OptionalValue {
                kind: self.subtype(0),
                value: None,
            }),

            _ => self.base.default_value(),
        }
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

pub fn resolve_rito_type(ctx: &mut Ctx<'_>, tree: &Cst) -> Result<RitoType, Diagnostic> {
    let mut c = tree.children.iter();

    let base = c.expect_token(TokenKind::Name)?;
    let base_span = base.span;

    let base =
        BinPropertyKind::from_ritobin_name(&ctx.text[base.span]).ok_or(UnknownType(base.span))?;

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
                    let resolved = BinPropertyKind::from_ritobin_name(&ctx.text[t.span]);
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

pub fn resolve_literal(
    ctx: &mut Ctx,
    tree: &Cst,
    kind_hint: Option<BinPropertyKind>,
) -> Result<Option<PropertyValueEnum>, Diagnostic> {
    use ltk_meta::value::*;
    use BinPropertyKind as K;
    use PropertyValueEnum as P;

    if tree.children.len() != 1 {
        return Ok(None);
    }
    Ok(Some(
        match tree.children.first().unwrap(/* checked above */) {
            cst::Child::Token(Token {
                kind: TokenKind::String,
                span,
            }) => P::String(StringValue(ctx.text[span].into())),
            cst::Child::Token(Token {
                kind: TokenKind::Number,
                span,
            }) => {
                let txt = &ctx.text[span];
                let Some(kind_hint) = kind_hint else {
                    return Ok(None);
                };

                dbg!(kind_hint);
                match kind_hint {
                    K::U8 => P::U8(U8Value(
                        txt.parse().map_err(|_| Diagnostic::ResolveLiteral)?,
                    )),
                    _ => return Ok(None),
                }
            }
            _ => return Ok(None),
        },
    ))
}

pub fn resolve_entry(
    ctx: &mut Ctx,
    tree: &Cst,
    parent_value_kind: Option<RitoType>,
) -> Result<IrEntry, MaybeSpanDiag> {
    let mut c = tree.children.iter();

    let key = c.expect_tree(Kind::EntryKey)?;

    let parent_value_kind = parent_value_kind
        .and_then(|p| p.value_subtype())
        .map(RitoType::simple);

    let kind = c
        .clone()
        .find_map(|c| c.tree().filter(|t| t.kind == Kind::TypeExpr))
        .map(|t| resolve_rito_type(ctx, t))
        .transpose()?
        .or(parent_value_kind);

    let value = c.expect_tree(Kind::EntryValue)?;
    let value_span = value.span;
    let literal = resolve_literal(ctx, tree, kind.map(|k| k.base))?;
    // let inferred_value = match value.children.first() {
    //     Some(cst::Child::Token(Token {
    //         kind: TokenKind::String,
    //         span,
    //         ..
    //     })) => Some(PropertyValueEnum::String(ltk_meta::value::StringValue(
    //         ctx.text[span].into(),
    //     ))),
    //     _ => None,
    // };

    let value = match (kind, literal) {
        (None, Some(value)) => value,
        (None, None) => return Err(MissingType(key.span).into()),
        (Some(kind), Some(ivalue)) if ivalue.kind() == kind.base => ivalue,
        (Some(kind), _) => kind.make_default(),
    };

    Ok(IrEntry {
        key: PropertyValueEnum::String(StringValue(ctx.text[key.span].into())).with_span(key.span),
        value: value.with_span(value_span),
    })
}

pub fn resolve_list(ctx: &mut Ctx, tree: &Cst) -> Result<(), Diagnostic> {
    Ok(())
}

impl TypeChecker<'_> {
    fn merge_ir(&mut self, mut parent: IrItem, child: IrItem) -> IrItem {
        if parent.value().kind().subtype_count() == 0 {
            // eprintln!("cant inject into non container");
            return parent;
        }
        match &mut parent.value_mut().inner {
            PropertyValueEnum::Container(list)
            | PropertyValueEnum::UnorderedContainer(UnorderedContainerValue(list)) => {
                let IrItem::ListItem(IrListItem(value)) = child else {
                    // eprintln!("list item must be list item");
                    return parent;
                };
                if list.item_kind != value.kind() {
                    // eprintln!(
                    //     "container kind mismatch {:?} / {:?}",
                    //     list.item_kind,
                    //     value.kind()
                    // );
                    return parent;
                }
                list.items.push(value.inner); // FIXME: span info inside all containers??
            }
            PropertyValueEnum::Struct(struct_value) => todo!(),
            PropertyValueEnum::Embedded(embedded_value) => todo!(),
            PropertyValueEnum::ObjectLink(object_link_value) => todo!(),
            PropertyValueEnum::Map(map_value) => {
                let IrItem::Entry(IrEntry { key, value }) = child else {
                    // eprintln!("map item must be entry");
                    return parent;
                };
                if map_value.value_kind != value.kind() {
                    // eprintln!(
                    //     "map value kind mismatch {:?} / {:?}",
                    //     map_value.value_kind,
                    //     value.kind()
                    // );
                    return parent;
                }
                map_value.entries.insert(
                    ltk_meta::value::PropertyValueUnsafeEq(key.inner),
                    value.inner,
                );
            }
            _ => unreachable!("non container"),
        }
        parent
    }
}

impl Visitor for TypeChecker<'_> {
    fn enter_tree(&mut self, tree: &Cst) -> Visit {
        self.depth += 1;
        let indent = "  ".repeat(self.depth.saturating_sub(1) as _);
        // eprintln!("{indent}> d:{} | {:?}", self.depth, tree.kind);
        // eprintln!("{indent}> stack: {:?}", &self.stack);

        let parent = self.stack.last();

        match tree.kind {
            Kind::ErrorTree => return Visit::Skip,

            Kind::Entry => {
                match resolve_entry(&mut self.ctx, tree, parent.map(|p| p.1.value().rito_type()))
                    .map_err(|e| e.fallback(tree.span))
                {
                    Ok(entry) => {
                        // eprintln!("entry: {entry:?}");
                        self.stack.push((self.depth, IrItem::Entry(entry)));
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
        //                 if self.depth == 2 {
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
        self.depth -= 1;
        let indent = "  ".repeat(self.depth.saturating_sub(1) as _);
        // eprintln!("{indent}< d:{} | {:?}", self.depth, tree.kind);
        // eprintln!("{indent}< stack: {:?}", &self.stack);
        if tree.kind == cst::Kind::ErrorTree {
            return Visit::Continue;
        }

        match self.stack.pop() {
            Some(ir) => {
                if ir.0 != self.depth + 1 {
                    self.stack.push(ir);
                    return Visit::Continue;
                }
                match self.stack.pop() {
                    Some(parent) => {
                        let parent = self.merge_ir(parent.1, ir.1);
                        self.stack.push((self.depth, parent));
                    }
                    None => {
                        let (
                            _,
                            IrItem::Entry(IrEntry {
                                key:
                                    Spanned {
                                        span: key_span,
                                        inner: PropertyValueEnum::String(StringValue(key)),
                                    },
                                value,
                            }),
                        ) = ir.clone()
                        else {
                            match self.depth {
                                1 => self
                                    .ctx
                                    .diagnostics
                                    .push(RootNonEntry.default_span(tree.span)),
                                _ => {
                                    self.stack.push(ir);
                                }
                            }
                            return Visit::Continue;
                        };
                        if let Some(existing) = self.root.insert(key, value) {
                            self.ctx.diagnostics.push(
                                ShadowedEntry {
                                    shadowee: existing.span,
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
