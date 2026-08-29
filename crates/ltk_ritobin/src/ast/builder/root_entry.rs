use crate::{
    ast::{diagnostics::Diagnostic as D, node::TypeExpr, Ptr, RootEntry, Value},
    cst::Kind,
    parse::Span,
    RitoType, Spanned,
};

use super::*;

mod root_kind;
use indexmap::IndexMap;
use ltk_meta::PropertyKind;
pub use root_kind::*;

#[derive(Debug, Clone)]
pub struct RawRootEntry {
    key: Value,
    type_expr: Spanned<Option<TypeExpr>>,
    value: Option<Value>,
}

impl<'a> Builder<'a> {
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
                        let key_span = entry.key.span();
                        let kind = RootKindOrUnknown::from_value(self.text, &entry.key);
                        if let Some(existing) = roots.insert(
                            kind,
                            RawRootEntry {
                                key: entry.key,
                                type_expr: entry.type_expr,
                                value: entry.value,
                            },
                        ) {
                            self.push(
                                D::ShadowedEntry {
                                    shadowee: existing.key.span(),
                                    shadower: key_span,
                                }
                                .unwrap(),
                            );
                        }
                    }
                    Err(e) => self.push(e.fallback(node.span)),
                },
                _ => self.push(D::RootNonEntry.default_span(node.span)),
            }
        }

        self.collect_root(roots)
    }

    fn take_root_value(
        &mut self,
        root_kind: RootKind,
        entry: RawRootEntry,
        expected: PropertyKind,
        extract: impl FnOnce(Value) -> Result<Value, Value>,
    ) -> Option<Value> {
        match extract(entry.value?) {
            Ok(v) => Some(v),
            Err(got) => {
                self.push(
                    D::InvalidRootEntryType {
                        root_kind,
                        key_span: entry.key.span(),
                        type_span: entry.type_expr.span,
                        got: got.kind().map(RitoType::simple).into(),
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
                D::MissingRootEntry {
                    root_kind: RootKind::Linked,
                }
                .default_span(Span::default()),
            );
        }
        let dependencies = dependencies
            .and_then(|e| {
                self.take_root_value(RootKind::Linked, e, PropertyKind::Container, |v| match v {
                    Value::Container { .. } => Ok(v),
                    other => Err(other),
                })
            })
            .map(|v| match v {
                Value::Container { items, .. } => items
                    .into_iter()
                    .filter_map(|item| {
                        let span = item.span();
                        match item {
                            Value::String(_) => Some(span),
                            other => {
                                self.push(
                                    D::UnexpectedContainerItem {
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
                D::MissingRootEntry {
                    root_kind: RootKind::Entries,
                }
                .default_span(Span::default()),
            );
        }
        let objects = objects
            .and_then(|e| {
                self.take_root_value(RootKind::Entries, e, PropertyKind::Map, |v| match v {
                    Value::Map { .. } => Ok(v),
                    other => Err(other),
                })
            })
            .map(|v| match v {
                Value::Map { entries, .. } => entries
                    .into_iter()
                    .filter_map(|(key, value)| {
                        let Value::Hash(path_hash) = key else {
                            return None;
                        };
                        match value {
                            Some(Value::Embedded(s)) => Some(RootEntry {
                                path_hash,
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
                        Value::String(_) => Ok(v),
                        other => Err(other),
                    })
                {
                    let Value::String(s) = &v else { unreachable!() };
                    match s.value.as_str() {
                        "PROP" => {}
                        "PTCH" => self.push(
                            D::CustomSpan("Patch bins are not supported yet", s.span).unwrap(),
                        ),
                        _ => self.push(D::CustomSpan("Unknown bin type", s.span).unwrap()),
                    }
                    bin_type = Some(s.span);
                }
            }
            None => self.push(
                D::MissingRootEntry {
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
                        Value::U32(_) => Ok(v),
                        other => Err(other),
                    })
                {
                    let Value::U32(n) = &v else { unreachable!() };
                    if n.value != 3 {
                        self.push(D::CustomSpan("Bin version should be '3'", n.meta).unwrap());
                    }
                    version = Some(Spanned::new(n.meta, n.value));
                }
            }
            None => self.push(
                D::MissingRootEntry {
                    root_kind: RootKind::Version,
                }
                .default_span(Span::default()),
            ),
        }

        for (_, unknown) in roots {
            self.push(
                D::UnknownRoot {
                    span: unknown.key.span(),
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
