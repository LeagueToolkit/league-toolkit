//! Walks a [`Cst`] directly and produces a fully resolved [`Ast`] in one recursive pass - the
//! second typechecker, parallel to (not sharing a pipeline with) [`crate::typecheck`]. See the
//! [`crate::ast`] module docs for why this exists as a separate engine.

use indexmap::IndexMap;
use ltk_hash::{BinHash, Hash as _};
use ltk_meta::{property::values, traits::PropertyExt as _, PropertyKind, PropertyValueEnum};

use crate::{
    ast::nodes::{AstProperty, AstStruct, AstValue, Ptr, Spanned},
    cst::{Child, Cst, Kind, Node},
    literals::{resolve_hash, resolve_literal, CanCoerce as _, CoerceFrom as _},
    parse::{Span, Token, TokenKind},
    typecheck::{
        diagnostics::{Diagnostic, DiagnosticWithSpan, MaybeSpanDiag, RitoTypeOrVirtual, RootKind},
        state::RootKindOrUnknown,
    },
    PropertyValueExt as _, RitoType, RitobinName as _,
};

use Diagnostic::*;

pub struct Ast {
    pub bin_type: Option<Span>,
    pub version: Option<Spanned<u32>>,
    pub dependencies: Vec<Span>,
    pub objects: Vec<AstObject>,
    pub diagnostics: Vec<DiagnosticWithSpan>,
}

pub struct AstObject {
    pub path_hash: Spanned<BinHash>,
    pub object: Ptr<AstStruct>,
}

/// Walks `cst` once and produces the fully resolved tree - the direct counterpart to
/// [`crate::Cst::build_bin`], just targeting [`Ast`]'s node types (which, unlike
/// `ltk_meta`'s, retain property-name spans) instead of merging into `ltk_meta::Bin` directly.
pub fn build(cst: &Cst, text: &str) -> Ast {
    let mut ctx = BuildCtx {
        cst,
        text,
        diagnostics: Vec::new(),
    };
    ctx.build_root()
}

struct RawRootEntry {
    key: PropertyValueEnum<Span>,
    type_span: Span,
    value: AstValue,
}

pub(super) struct BuildCtx<'a> {
    cst: &'a Cst,
    text: &'a str,
    diagnostics: Vec<DiagnosticWithSpan>,
}

/// Mirrors `typecheck::resolve::TreeIterExt` (the same "find a child of this kind" need, just
/// returning `Option` instead of erroring directly, since callers here want a range of
/// fallbacks/diagnostics depending on context).
trait ChildrenExt {
    fn find_tree<'c>(&'c self, cst: &'c Cst, kind: Kind) -> Option<&'c Node>;
    fn find_token<'c>(&'c self, cst: &'c Cst, kind: TokenKind) -> Option<&'c Token>;
}

impl ChildrenExt for [Child] {
    fn find_tree<'c>(&'c self, cst: &'c Cst, kind: Kind) -> Option<&'c Node> {
        self.iter()
            .find_map(|c| c.tree(cst).filter(|t| t.kind == kind))
    }
    fn find_token<'c>(&'c self, cst: &'c Cst, kind: TokenKind) -> Option<&'c Token> {
        self.iter()
            .find_map(|c| c.token(cst).filter(|t| t.kind == kind))
    }
}

impl<'a> BuildCtx<'a> {
    pub(super) fn cst(&self) -> &'a Cst {
        self.cst
    }

    fn push(&mut self, d: DiagnosticWithSpan) {
        self.diagnostics.push(d);
    }

    /// Resolves a single scalar list item (a bare number) against `expected` - used by
    /// [`crate::ast::listlikes`] to pull the `f32`/`u8` components of a `vec2`/`vec3`/`vec4`/
    /// `rgba`/`mtx44`'s flat body. `node` is the `ListItem` wrapping the literal.
    pub(super) fn resolve_scalar(
        &mut self,
        node: &Node,
        expected: PropertyKind,
        hint_span: Option<Span>,
    ) -> Result<AstValue, Diagnostic> {
        match self.resolve_value(node, Some(RitoType::simple(expected)), hint_span)? {
            Some(v) => Ok(v),
            None => Err(TypeMismatch {
                span: node.span,
                expected: RitoType::simple(expected),
                expected_span: hint_span,
                got: RitoTypeOrVirtual::numeric(),
            }),
        }
    }

    // ---- type expressions -------------------------------------------------------------------

    fn resolve_type_expr(&mut self, tree: &Node) -> Result<RitoType, Diagnostic> {
        let children = tree.children.get(self.cst);

        let base = children
            .find_token(self.cst, TokenKind::Name)
            .ok_or(MissingToken(TokenKind::Name))?;
        let base_span = base.span;
        let base =
            PropertyKind::from_rito_name(&self.text[base.span]).ok_or(UnknownType(base.span))?;

        let subtypes = match children.find_tree(self.cst, Kind::TypeArgList) {
            Some(subtypes_node) => {
                let subtypes_span = subtypes_node.span;
                let expected = base.subtype_count();

                if expected == 0 {
                    return Err(UnexpectedSubtypes {
                        span: subtypes_span,
                        base_type: base_span,
                    });
                }

                let subtypes = subtypes_node
                    .children
                    .get(self.cst)
                    .iter()
                    .filter_map(|c| c.tree(self.cst).filter(|t| t.kind == Kind::TypeArg))
                    .map(|t| {
                        let resolved = PropertyKind::from_rito_name(&self.text[t.span]);
                        if resolved.is_none() {
                            self.push(UnknownType(t.span).unwrap());
                        }
                        (resolved, t.span)
                    })
                    .collect::<Vec<_>>();

                if subtypes.len() != expected.into() {
                    let span = if subtypes.len() > expected.into() {
                        subtypes[expected as _..]
                            .iter()
                            .map(|s| s.1)
                            .reduce(|acc, s| Span::new(acc.start, s.end))
                            .unwrap_or(subtypes_span)
                    } else {
                        subtypes.last().map(|s| s.1).unwrap_or(subtypes_span)
                    };
                    return Err(SubtypeCountMismatch {
                        span,
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

    // ---- class names --------------------------------------------------------------------------

    fn resolve_class_hash(&mut self, token: &Token) -> Result<BinHash, Diagnostic> {
        match token {
            Token {
                kind: TokenKind::Name,
                span,
            } => Ok(BinHash::hash_str(&self.text[span])),
            Token {
                kind: TokenKind::HexLit,
                span,
            } => match resolve_hash(self.text, *span)? {
                PropertyValueEnum::Hash(hash) => Ok(*hash),
                value => Err(TypeMismatch {
                    span: *value.meta(),
                    expected: RitoType::simple(PropertyKind::Hash),
                    expected_span: None,
                    got: value.rito_type().into(),
                }),
            },
            _ => Err(InvalidHash(token.span)),
        }
    }

    // ---- values -------------------------------------------------------------------------------

    /// Resolves an `EntryValue` or `ListItem` node into the value it describes - both wrap a
    /// single `Class`/`Literal` child. A bare `{ .. }` used as a list item parses as its own
    /// `ListItemBlock` node instead (see [`Self::resolve_block`]), never a `ListItem` wrapping a
    /// `Block` - so unlike `Class`/`Literal`, `Block` never appears as this node's child.
    fn resolve_value(
        &mut self,
        wrapper: &Node,
        hint: Option<RitoType>,
        hint_span: Option<Span>,
    ) -> Result<Option<AstValue>, Diagnostic> {
        let Some(child) = wrapper.children.get(self.cst).first() else {
            return Ok(None);
        };
        let Some(node) = child.tree(self.cst) else {
            return Ok(None);
        };
        match node.kind {
            Kind::Class => {
                let Some(hint) = hint else { return Ok(None) };
                self.resolve_class(node, hint).map(Some)
            }
            Kind::Block => {
                let Some(hint) = hint else { return Ok(None) };
                if matches!(hint.base, PropertyKind::Struct | PropertyKind::Embedded) {
                    return Err(MissingClassName {
                        span: node.open_brace_span(self.cst),
                        expected: hint,
                    });
                }
                self.resolve_block(node, hint, hint_span)
                    .map(Some)
                    .map_err(|e| e.fallback(node.span).diagnostic)
            }
            Kind::Literal => {
                let Some(token_child) = node.children.get(self.cst).first() else {
                    return Ok(None);
                };
                let Some(token) = token_child.token(self.cst) else {
                    return Ok(None);
                };
                resolve_literal(self.text, token, hint, hint_span).map(|v| v.map(AstValue::from))
            }
            _ => Ok(None),
        }
    }

    /// Same as [`Self::resolve_value`], but for a `ListItemBlock` node directly (a bare `{ .. }`
    /// occupying a list-item position - already the value-bearing node, nothing to unwrap).
    fn resolve_list_item_block(
        &mut self,
        node: &Node,
        hint: RitoType,
        hint_span: Option<Span>,
    ) -> Result<AstValue, MaybeSpanDiag> {
        self.resolve_block(node, hint, hint_span)
    }

    fn resolve_class(&mut self, class: &Node, hint: RitoType) -> Result<AstValue, Diagnostic> {
        let children = class.children.get(self.cst);
        let Some(name_token) = children.first().and_then(|c| c.token(self.cst)) else {
            return Err(InvalidHash(class.span));
        };
        let class_hash = self.resolve_class_hash(name_token)?;

        if !matches!(hint.base, PropertyKind::Struct | PropertyKind::Embedded) {
            return Err(TypeMismatch {
                span: name_token.span,
                expected: RitoType::simple(hint.base),
                expected_span: None,
                got: RitoTypeOrVirtual::StructOrEmbedded,
            });
        }

        let properties = match children.find_tree(self.cst, Kind::Block) {
            Some(block) => self.resolve_body_properties(block, hint),
            None => Vec::new(),
        };

        let ast_struct = AstStruct {
            class_hash: Spanned::new(name_token.span, class_hash),
            span: class.span,
            properties,
        };

        Ok(match hint.base {
            PropertyKind::Struct => AstValue::Struct(ast_struct),
            PropertyKind::Embedded => AstValue::Embedded(ast_struct),
            _ => unreachable!(),
        })
    }

    /// Resolves a `Block`/`ListItemBlock` node's body against `hint` - dispatches on `hint.base`
    /// since the same body shape (`Entry`/`ListItem`/`ListItemBlock`/`Comment` children) means
    /// different things depending on what's being populated.
    fn resolve_block(
        &mut self,
        block: &Node,
        hint: RitoType,
        hint_span: Option<Span>,
    ) -> Result<AstValue, MaybeSpanDiag> {
        use PropertyKind as K;

        match hint.base {
            K::Vector2 | K::Vector3 | K::Vector4 | K::Color | K::Matrix44 => {
                self.resolve_listlike(block, hint.base, hint_span)
            }
            K::Map => {
                let key_kind = hint.subtype(0);
                let value_kind = hint.subtype(1);
                let entries = self.resolve_body_map_entries(block, key_kind, value_kind, hint_span);
                Ok(AstValue::Map {
                    key_kind,
                    value_kind,
                    entries,
                    span: block.span,
                })
            }
            K::Container | K::UnorderedContainer => {
                let item_kind = hint.subtype(0);
                let items = self.resolve_body_items(block, item_kind, hint_span);
                Ok(match hint.base {
                    K::Container => AstValue::Container {
                        item_kind,
                        items,
                        span: block.span,
                    },
                    K::UnorderedContainer => AstValue::UnorderedContainer {
                        item_kind,
                        items,
                        span: block.span,
                    },
                    _ => unreachable!(),
                })
            }
            K::Optional => {
                let item_kind = hint.subtype(0);
                let item_hint = RitoType::simple(item_kind);
                if matches!(
                    item_kind,
                    K::Vector2 | K::Vector3 | K::Vector4 | K::Color | K::Matrix44
                ) {
                    // an optional listlike spells its components flat, same as a bare listlike
                    let has_any = block
                        .children
                        .get(self.cst)
                        .iter()
                        .any(|c| c.tree(self.cst).is_some_and(|n| n.kind != Kind::Comment));
                    if !has_any {
                        return Ok(AstValue::Optional {
                            item_kind,
                            value: None,
                            span: block.span,
                        });
                    }
                    let inner = self.resolve_listlike(block, item_kind, hint_span)?;
                    return Ok(AstValue::Optional {
                        item_kind,
                        value: Some(Box::new(inner)),
                        span: block.span,
                    });
                }
                let mut value = None;
                for child in block.children.get(self.cst).iter() {
                    let Some(node) = child.tree(self.cst) else {
                        continue;
                    };
                    match node.kind {
                        Kind::Comment => continue,
                        Kind::ListItem => {
                            match self.resolve_value(node, Some(item_hint), hint_span) {
                                Ok(Some(v)) => value = Some(v),
                                Ok(None) => {}
                                Err(e) => self.push(e.default_span(node.span)),
                            }
                        }
                        Kind::ListItemBlock => {
                            match self.resolve_list_item_block(node, item_hint, hint_span) {
                                Ok(v) => value = Some(v),
                                Err(e) => self.push(e.fallback(node.span)),
                            }
                        }
                        _ => self.push(
                            UnexpectedItem {
                                span: node.span,
                                parent: hint,
                                expected: crate::ItemShape::Value,
                            }
                            .unwrap(),
                        ),
                    }
                }
                Ok(AstValue::Optional {
                    item_kind,
                    value: value.map(Box::new),
                    span: block.span,
                })
            }
            _ => Err(UnexpectedContainerItem {
                span: block.span,
                expected: hint,
                expected_span: hint_span,
            }
            .into()),
        }
    }

    // ---- bodies -------------------------------------------------------------------------------

    fn resolve_body_properties(&mut self, block: &Node, hint: RitoType) -> Vec<AstProperty> {
        let mut properties = Vec::new();
        for child in block.children.get(self.cst).iter() {
            let Some(node) = child.tree(self.cst) else {
                continue;
            };
            match node.kind {
                Kind::Comment => continue,
                Kind::Entry => match self.resolve_entry(node, Some(hint), None) {
                    Ok(entry) => {
                        let key_span = *entry.key.meta();
                        let key_kind = entry.key.rito_type();
                        match PropertyKind::Hash.coerce_from(entry.key) {
                            Some(PropertyValueEnum::Hash(hash)) => properties.push(AstProperty {
                                name: Spanned::new(key_span, *hash),
                                type_span: entry.type_span,
                                value: entry.value,
                            }),
                            _ => self.push(
                                TypeMismatch {
                                    span: key_span,
                                    expected: RitoType::simple(PropertyKind::Hash),
                                    expected_span: None,
                                    got: key_kind.into(),
                                }
                                .unwrap(),
                            ),
                        }
                    }
                    Err(e) => self.push(e.fallback(node.span)),
                },
                Kind::ListItem | Kind::ListItemBlock => self.push(
                    UnexpectedItem {
                        span: node.span,
                        parent: hint,
                        expected: crate::ItemShape::Entry,
                    }
                    .unwrap(),
                ),
                _ => {}
            }
        }
        properties
    }

    fn resolve_body_map_entries(
        &mut self,
        block: &Node,
        key_kind: PropertyKind,
        value_kind: PropertyKind,
        hint_span: Option<Span>,
    ) -> Vec<(AstValue, AstValue)> {
        let hint = RitoType::map(key_kind, value_kind);
        let mut entries = Vec::new();
        for child in block.children.get(self.cst).iter() {
            let Some(node) = child.tree(self.cst) else {
                continue;
            };
            match node.kind {
                Kind::Comment => continue,
                Kind::Entry => match self.resolve_entry(node, Some(hint), hint_span) {
                    Ok(entry) => {
                        let key_span = *entry.key.meta();
                        let got_key_kind = entry.key.rito_type();
                        match key_kind.coerce_from(entry.key) {
                            Some(key) => entries.push((AstValue::from(key), entry.value)),
                            None => self.push(
                                TypeMismatch {
                                    span: key_span,
                                    expected: RitoType::simple(key_kind),
                                    expected_span: hint_span,
                                    got: got_key_kind.into(),
                                }
                                .unwrap(),
                            ),
                        }
                    }
                    Err(e) => self.push(e.fallback(node.span)),
                },
                Kind::ListItem | Kind::ListItemBlock => self.push(
                    UnexpectedItem {
                        span: node.span,
                        parent: hint,
                        expected: crate::ItemShape::Entry,
                    }
                    .unwrap(),
                ),
                _ => {}
            }
        }
        entries
    }

    fn resolve_body_items(
        &mut self,
        block: &Node,
        item_kind: PropertyKind,
        hint_span: Option<Span>,
    ) -> Vec<AstValue> {
        let item_hint = RitoType::simple(item_kind);
        let mut items = Vec::new();
        for child in block.children.get(self.cst).iter() {
            let Some(node) = child.tree(self.cst) else {
                continue;
            };
            match node.kind {
                Kind::Comment => continue,
                Kind::ListItem => match self.resolve_value(node, Some(item_hint), hint_span) {
                    Ok(Some(mut v)) => {
                        if v.kind() != item_kind {
                            if let Some(coerced) = v.clone().coerce_to(item_kind) {
                                v = coerced;
                            }
                        }
                        items.push(v);
                    }
                    Ok(None) => {}
                    Err(e) => self.push(e.default_span(node.span)),
                },
                Kind::ListItemBlock => {
                    match self.resolve_list_item_block(node, item_hint, hint_span) {
                        Ok(v) => items.push(v),
                        Err(e) => self.push(e.fallback(node.span)),
                    }
                }
                Kind::Entry => self.push(
                    UnexpectedItem {
                        span: node.span,
                        parent: RitoType::container(item_kind),
                        expected: crate::ItemShape::Value,
                    }
                    .unwrap(),
                ),
                _ => {}
            }
        }
        items
    }

    // ---- entries ------------------------------------------------------------------------------

    fn resolve_entry_key(
        &mut self,
        key_node: &Node,
        parent_value_kind: Option<RitoType>,
        parent_type_span: Option<Span>,
    ) -> Result<PropertyValueEnum<Span>, Diagnostic> {
        let token = key_node
            .children
            .get(self.cst)
            .first()
            .ok_or(InvalidHash(key_node.span))?
            .token(self.cst);

        Ok(match token {
            Some(Token {
                kind: TokenKind::Name,
                span,
            }) => PropertyValueEnum::from(values::String::new_with_meta(
                self.text[span].into(),
                *span,
            )),
            Some(Token {
                kind: TokenKind::String,
                span,
            }) => {
                if let Some(parent) = parent_value_kind
                    .filter(|p| matches!(p.base, PropertyKind::Struct | PropertyKind::Embedded))
                {
                    self.push(
                        QuotedPropertyName {
                            span: *span,
                            parent,
                        }
                        .unwrap(),
                    );
                }
                PropertyValueEnum::from(values::String::new_with_meta(
                    self.text[Span::new(span.start + 1, span.end - 1)].into(),
                    *span,
                ))
            }
            Some(Token {
                kind: TokenKind::HexLit,
                span,
            }) => resolve_hash(self.text, *span)?,
            Some(token) => resolve_literal(
                self.text,
                token,
                parent_value_kind
                    .and_then(|k| k.subtypes[0])
                    .map(RitoType::simple),
                parent_type_span,
            )?
            .ok_or(CustomSpan("erm idk bad literal", key_node.span))?,
            None => return Err(InvalidHash(key_node.span)),
        })
    }

    /// Resolves an `Entry` node into the key/type/value it describes - mirrors
    /// `typecheck::resolve::resolve_entry`'s rules exactly (that's deliberate: same rules, this
    /// engine just reads them off the CST directly rather than through the CST `Visitor`), but
    /// recurses into the value's own body itself rather than relying on an external walker to
    /// merge children in afterward.
    fn resolve_entry(
        &mut self,
        entry: &Node,
        parent_value_kind: Option<RitoType>,
        parent_type_span: Option<Span>,
    ) -> Result<RawEntry, MaybeSpanDiag> {
        let children = entry.children.get(self.cst);
        let key_node = children
            .find_tree(self.cst, Kind::EntryKey)
            .ok_or(MissingTree(Kind::EntryKey))?;
        let key = self.resolve_entry_key(key_node, parent_value_kind, parent_type_span)?;

        let parent_value_kind = parent_value_kind
            .and_then(|p| p.value_subtype())
            .map(RitoType::simple);

        let kind_node = children.find_tree(self.cst, Kind::TypeExpr);
        let kind_span = kind_node.map(|k| k.span);
        let kind = kind_node.map(|t| self.resolve_type_expr(t)).transpose()?;

        let value_node = children
            .find_tree(self.cst, Kind::EntryValue)
            .ok_or(MissingTree(Kind::EntryValue))?;
        let value_span = value_node.span;

        if let Some(parent) = parent_value_kind.as_ref() {
            if let Some((kind, kind_span)) = kind.as_ref().zip(kind_span) {
                if !parent.can_coerce(*kind) {
                    self.push(
                        TypeMismatch {
                            span: kind_span,
                            expected: *parent,
                            expected_span: parent_type_span,
                            got: (*kind).into(),
                        }
                        .unwrap(),
                    );
                    return Ok(RawEntry {
                        key,
                        type_span: parent_type_span,
                        value: AstValue::default_for(*parent, value_span),
                    });
                }
            }
        }

        let kind = kind.or(parent_value_kind);
        let type_span = kind_span.or(parent_type_span);

        let resolved_val = match self.resolve_value(value_node, kind, type_span) {
            Ok(v) => v,
            Err(e) => match kind {
                Some(kind) => {
                    self.push(e.default_span(entry.span));
                    Some(AstValue::default_for(kind, value_span))
                }
                None => return Err(e.into()),
            },
        };

        let resolved_val = resolved_val.map(|value| match kind {
            Some(kind) if value.kind() == kind.base => value,
            Some(kind) => value.clone().coerce_to(kind.base).unwrap_or(value),
            None => value,
        });

        let value = match (kind, resolved_val) {
            (None, Some(value)) => value,
            (None, None) => return Err(MissingType(*key.meta()).into()),
            (Some(kind), Some(value)) => match value.kind() == kind.base {
                true => value,
                false => {
                    return Err(TypeMismatch {
                        span: value.span(),
                        expected: kind,
                        expected_span: kind_span,
                        got: value.rito_type().into(),
                    }
                    .into())
                }
            },
            (Some(kind), None) => AstValue::default_for(kind, value_span),
        };

        Ok(RawEntry {
            key,
            value,
            type_span,
        })
    }

    // ---- root -----------------------------------------------------------------------------

    fn build_root(&mut self) -> Ast {
        let root_node = self.cst.root();
        let mut roots: IndexMap<RootKindOrUnknown, RawRootEntry> = IndexMap::new();

        for child in root_node.children.get(self.cst).iter() {
            let Some(node) = child.tree(self.cst) else {
                continue;
            };
            match node.kind {
                Kind::Comment | Kind::ErrorTree => continue,
                Kind::Entry => match self.resolve_entry(node, None, None) {
                    Ok(entry) => {
                        let key_span = *entry.key.meta();
                        let kind = RootKindOrUnknown::from_value(self.text, &entry.key);
                        if let Some(existing) = roots.insert(
                            kind,
                            RawRootEntry {
                                key: entry.key,
                                type_span: entry.type_span.unwrap_or(key_span),
                                value: entry.value,
                            },
                        ) {
                            self.push(
                                ShadowedEntry {
                                    shadowee: *existing.key.meta(),
                                    shadower: key_span,
                                }
                                .unwrap(),
                            );
                        }
                    }
                    Err(e) => self.push(e.fallback(node.span)),
                },
                _ => self.push(RootNonEntry.default_span(node.span)),
            }
        }

        self.collect_root(roots)
    }

    fn take_root_value(
        &mut self,
        root_kind: RootKind,
        entry: RawRootEntry,
        expected: PropertyKind,
        extract: impl FnOnce(AstValue) -> Result<AstValue, AstValue>,
    ) -> Option<AstValue> {
        match extract(entry.value) {
            Ok(v) => Some(v),
            Err(got) => {
                self.push(
                    InvalidRootEntryType {
                        root_kind,
                        key_span: *entry.key.meta(),
                        type_span: entry.type_span,
                        got: RitoType::simple(got.kind()),
                        expected: RitoType::simple(expected),
                    }
                    .unwrap(),
                );
                None
            }
        }
    }

    fn collect_root(&mut self, mut roots: IndexMap<RootKindOrUnknown, RawRootEntry>) -> Ast {
        let dependencies = roots.swap_remove(&RootKindOrUnknown::Known(RootKind::Linked));
        if dependencies.is_none() {
            self.push(
                MissingRootEntry {
                    root_kind: RootKind::Linked,
                }
                .default_span(Span::default()),
            );
        }
        let dependencies = dependencies
            .and_then(|e| {
                self.take_root_value(RootKind::Linked, e, PropertyKind::Container, |v| match v {
                    AstValue::Container { .. } => Ok(v),
                    other => Err(other),
                })
            })
            .map(|v| match v {
                AstValue::Container { items, .. } => items
                    .into_iter()
                    .filter_map(|item| {
                        let span = item.span();
                        match item {
                            AstValue::String(_) => Some(span),
                            other => {
                                self.push(
                                    UnexpectedContainerItem {
                                        span,
                                        expected: RitoType::simple(PropertyKind::String),
                                        expected_span: None,
                                    }
                                    .unwrap(),
                                );
                                let _ = other;
                                None
                            }
                        }
                    })
                    .collect::<Vec<_>>(),
                _ => unreachable!(),
            })
            .unwrap_or_default();

        let objects = roots.swap_remove(&RootKindOrUnknown::Known(RootKind::Entries));
        if objects.is_none() {
            self.push(
                MissingRootEntry {
                    root_kind: RootKind::Entries,
                }
                .default_span(Span::default()),
            );
        }
        let objects = objects
            .and_then(|e| {
                self.take_root_value(RootKind::Entries, e, PropertyKind::Map, |v| match v {
                    AstValue::Map { .. } => Ok(v),
                    other => Err(other),
                })
            })
            .map(|v| match v {
                AstValue::Map { entries, .. } => entries
                    .into_iter()
                    .filter_map(|(key, value)| {
                        let AstValue::Hash(path_hash) = key else {
                            return None;
                        };
                        match value {
                            AstValue::Embedded(s) => Some(AstObject {
                                path_hash: Spanned::new(path_hash.meta, *path_hash),
                                object: Ptr::new(s),
                            }),
                            _ => None,
                        }
                    })
                    .collect::<Vec<_>>(),
                _ => unreachable!(),
            })
            .unwrap_or_default();

        let mut bin_type = None;
        match roots.swap_remove(&RootKindOrUnknown::Known(RootKind::Type)) {
            Some(e) => {
                if let Some(v) =
                    self.take_root_value(RootKind::Type, e, PropertyKind::String, |v| match v {
                        AstValue::String(_) => Ok(v),
                        other => Err(other),
                    })
                {
                    let AstValue::String(s) = &v else {
                        unreachable!()
                    };
                    match s.value.as_str() {
                        "PROP" => {}
                        "PTCH" => self
                            .push(CustomSpan("Patch bins are not supported yet", s.meta).unwrap()),
                        _ => self.push(CustomSpan("Unknown bin type", s.meta).unwrap()),
                    }
                    bin_type = Some(s.meta);
                }
            }
            None => self.push(
                MissingRootEntry {
                    root_kind: RootKind::Type,
                }
                .default_span(Span::default()),
            ),
        }

        let mut version = None;
        match roots.swap_remove(&RootKindOrUnknown::Known(RootKind::Version)) {
            Some(e) => {
                if let Some(v) =
                    self.take_root_value(RootKind::Version, e, PropertyKind::U32, |v| match v {
                        AstValue::U32(_) => Ok(v),
                        other => Err(other),
                    })
                {
                    let AstValue::U32(n) = &v else { unreachable!() };
                    if n.value != 3 {
                        self.push(CustomSpan("Bin version should be '3'", n.meta).unwrap());
                    }
                    version = Some(Spanned::new(n.meta, n.value));
                }
            }
            None => self.push(
                MissingRootEntry {
                    root_kind: RootKind::Version,
                }
                .default_span(Span::default()),
            ),
        }

        for (_, unknown) in roots {
            self.push(
                UnknownRoot {
                    span: *unknown.key.meta(),
                }
                .default_span(Span::default()),
            );
        }

        Ast {
            bin_type,
            version,
            dependencies,
            objects,
            diagnostics: std::mem::take(&mut self.diagnostics),
        }
    }
}

struct RawEntry {
    key: PropertyValueEnum<Span>,
    type_span: Option<Span>,
    value: AstValue,
}

impl AstValue {
    /// A default value of `kind`, spanned at `span` - used when resolution fails but a hint is
    /// available, so the walk can keep going (matches `RitoType::make_default`'s role in the
    /// existing engine).
    fn default_for(kind: RitoType, span: Span) -> AstValue {
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
                class_hash: Spanned::new(span, BinHash::default()),
                span,
                properties: Vec::new(),
            }),
            K::Embedded => AstValue::Embedded(AstStruct {
                class_hash: Spanned::new(span, BinHash::default()),
                span,
                properties: Vec::new(),
            }),
            other => AstValue::from({
                let mut v = other.default_value::<Span>();
                *v.meta_mut() = span;
                v
            }),
        }
    }

    /// Coerces this already-resolved value to `to`, mirroring `literals::CoerceFrom` (which
    /// operates on `PropertyValueEnum`) for the leaf kinds it covers.
    fn coerce_to(self, to: PropertyKind) -> Option<AstValue> {
        if self.kind() == to {
            return Some(self);
        }
        let leaf: PropertyValueEnum<Span> = self.try_into().ok()?;
        to.coerce_from(leaf).map(AstValue::from)
    }
}

impl From<PropertyValueEnum<Span>> for AstValue {
    fn from(value: PropertyValueEnum<Span>) -> Self {
        match value {
            PropertyValueEnum::None(v) => AstValue::None(v),
            PropertyValueEnum::Bool(v) => AstValue::Bool(v),
            PropertyValueEnum::BitBool(v) => AstValue::BitBool(v),
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
            PropertyValueEnum::Color(v) => AstValue::Color(v),
            PropertyValueEnum::String(v) => AstValue::String(v),
            PropertyValueEnum::Hash(v) => AstValue::Hash(v),
            PropertyValueEnum::WadChunkLink(v) => AstValue::WadChunkLink(v),
            PropertyValueEnum::ObjectLink(v) => AstValue::ObjectLink(v),
            PropertyValueEnum::Struct(s) => AstValue::Struct(AstStruct {
                class_hash: Spanned::new(s.meta, s.class_hash),
                span: s.meta,
                properties: Vec::new(),
            }),
            PropertyValueEnum::Embedded(values::Embedded(s)) => AstValue::Embedded(AstStruct {
                class_hash: Spanned::new(s.meta, s.class_hash),
                span: s.meta,
                properties: Vec::new(),
            }),
            PropertyValueEnum::Container(c) => AstValue::Container {
                item_kind: c.item_kind(),
                span: *c.meta(),
                items: Vec::new(),
            },
            PropertyValueEnum::UnorderedContainer(values::UnorderedContainer(c)) => {
                AstValue::UnorderedContainer {
                    item_kind: c.item_kind(),
                    span: *c.meta(),
                    items: Vec::new(),
                }
            }
            PropertyValueEnum::Map(m) => AstValue::Map {
                key_kind: m.key_kind(),
                value_kind: m.value_kind(),
                span: m.meta,
                entries: Vec::new(),
            },
            PropertyValueEnum::Optional(o) => AstValue::Optional {
                item_kind: o.item_kind(),
                span: *o.meta(),
                value: None,
            },
        }
    }
}

impl TryFrom<AstValue> for PropertyValueEnum<Span> {
    type Error = ();
    fn try_from(value: AstValue) -> Result<Self, ()> {
        Ok(match value {
            AstValue::None(v) => PropertyValueEnum::None(v),
            AstValue::Bool(v) => PropertyValueEnum::Bool(v),
            AstValue::BitBool(v) => PropertyValueEnum::BitBool(v),
            AstValue::I8(v) => PropertyValueEnum::I8(v),
            AstValue::U8(v) => PropertyValueEnum::U8(v),
            AstValue::I16(v) => PropertyValueEnum::I16(v),
            AstValue::U16(v) => PropertyValueEnum::U16(v),
            AstValue::I32(v) => PropertyValueEnum::I32(v),
            AstValue::U32(v) => PropertyValueEnum::U32(v),
            AstValue::I64(v) => PropertyValueEnum::I64(v),
            AstValue::U64(v) => PropertyValueEnum::U64(v),
            AstValue::F32(v) => PropertyValueEnum::F32(v),
            AstValue::Vector2(v) => PropertyValueEnum::Vector2(v),
            AstValue::Vector3(v) => PropertyValueEnum::Vector3(v),
            AstValue::Vector4(v) => PropertyValueEnum::Vector4(v),
            AstValue::Matrix44(v) => PropertyValueEnum::Matrix44(v),
            AstValue::Color(v) => PropertyValueEnum::Color(v),
            AstValue::String(v) => PropertyValueEnum::String(v),
            AstValue::Hash(v) => PropertyValueEnum::Hash(v),
            AstValue::WadChunkLink(v) => PropertyValueEnum::WadChunkLink(v),
            AstValue::ObjectLink(v) => PropertyValueEnum::ObjectLink(v),
            AstValue::Struct(_)
            | AstValue::Embedded(_)
            | AstValue::Container { .. }
            | AstValue::UnorderedContainer { .. }
            | AstValue::Map { .. }
            | AstValue::Optional { .. } => return Err(()),
        })
    }
}
