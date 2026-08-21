use indexmap::IndexMap;
use ltk_hash::BinHash;
use ltk_meta::{traits::PropertyExt as _, PropertyKind, PropertyValueEnum};

use crate::{
    ast::{
        nodes::{AstStruct, Ptr, Spanned},
        AstValue,
    },
    cst::{Child, Cst, Kind, Node},
    parse::{Span, Token, TokenKind},
    typecheck::{
        diagnostics::{Diagnostic, DiagnosticWithSpan, RootKind},
        state::RootKindOrUnknown,
    },
    RitoType,
};

use Diagnostic::*;

#[derive(Debug, Clone)]
pub struct Ast {
    pub bin_type: Option<Span>,
    pub version: Option<Spanned<u32>>,
    pub dependencies: Vec<Span>,
    pub objects: Vec<AstObject>,
    pub diagnostics: Vec<DiagnosticWithSpan>,
}

#[derive(Debug, Clone)]
pub struct AstObject {
    pub path_hash: Spanned<BinHash>,
    pub object: Ptr<AstStruct>,
}

impl Ast {
    pub fn from_cst(cst: &Cst, text: &str) -> Self {
        let mut ctx = BuildCtx {
            cst,
            text,
            diagnostics: Vec::new(),
        };
        ctx.build_root()
    }
}

#[derive(Debug, Clone)]
pub(super) struct RawRootEntry {
    key: PropertyValueEnum<Span>,
    type_span: Span,
    value: AstValue,
}

#[derive(Debug, Clone)]
pub(super) struct BuildCtx<'a> {
    pub cst: &'a Cst,
    pub text: &'a str,
    pub diagnostics: Vec<DiagnosticWithSpan>,
}

pub trait ChildrenExt {
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

    pub(super) fn push(&mut self, d: DiagnosticWithSpan) {
        self.diagnostics.push(d);
    }

    pub(crate) fn build_root(&mut self) -> Ast {
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

    pub(crate) fn take_root_value(
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

    pub(crate) fn collect_root(
        &mut self,
        mut roots: IndexMap<RootKindOrUnknown, RawRootEntry>,
    ) -> Ast {
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
