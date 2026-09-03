use crate::{
    ast::{
        diagnostics::Diagnostic as D,
        node::{
            root::{KnownRoot, Root, RootKind, RootValue},
            roots::Roots,
            TypeExpr,
        },
        RootEntry, Value,
    },
    cst::Kind,
    parse::Span,
    Node, Spanned, SpannedExt,
};

use super::*;

use ltk_meta::PropertyKind::{self};

#[derive(Debug, Clone)]
pub struct RawRootProperty {
    pub key: Spanned<RootKind>,
    pub type_expr: Spanned<Option<TypeExpr>>,
    pub value: Option<Value>,
}

impl<'a> Builder<'a> {
    pub(crate) fn build(mut self) -> Ast {
        let root_node = self.cst.root();

        let mut file_type = None;
        let mut version = None;
        let mut linked = None;
        let mut entries = None;

        let mut idx = 0;
        let mut roots = Roots::new(root_node.children.get(self.cst).iter().filter_map(|child| {
            let node = child.tree(self.cst)?;
            let root = self.resolve_root(node, idx)?;
            idx += 1;
            Some(root)
        }));

        // we don't need to coerce here since we collected these roots via Self::resolve_entry, who
        // handles coercion already

        for (idx, root) in roots.iter_mut().enumerate() {
            if let Some(expected_type) = root.name.expected_type() {
                match root.type_expr.value {
                    Some(type_expr) => {
                        if type_expr != expected_type {
                            self.push(
                                D::InvalidRootEntryType {
                                    root_kind: *root.name,
                                    key_span: root.name.span,
                                    type_span: root.type_expr.span,
                                    got: type_expr.into(),
                                    expected: expected_type,
                                }
                                .unwrap(),
                            );
                        }
                    }
                    None => {
                        self.push(
                            D::MissingEntryType {
                                key_span: root.name.span,
                            }
                            .unwrap(),
                        );
                    }
                }

                if let Some(RootValue::Value(value)) = root.value.as_ref() {
                    if let Some(got) = value.rito_type() {
                        if got != expected_type {
                            self.push(
                                D::TypeMismatch {
                                    span: value.span(),
                                    expected: expected_type.into(),
                                    expected_span: None,
                                    got: got.into(),
                                }
                                .unwrap(),
                            );
                            continue;
                        }
                    }
                }
            }
            match *root.name {
                RootKind::Unknown => {
                    self.push(
                        D::MissingEntryValue {
                            key_span: root.name.span,
                            expected: root
                                .name
                                .expected_type()
                                .map(|t| t.with_span(root.type_expr.span)),
                        }
                        .unwrap(),
                    );
                }
                RootKind::Version => {
                    if let Some(RootValue::Value(Value::U32(v))) = &root.value {
                        if let Some(existing) = version.replace(KnownRoot {
                            idx,
                            value: v.value,
                        }) {
                            self.push(
                                D::ShadowedRoot {
                                    shadower: idx,
                                    shadowee: existing.idx,
                                }
                                .default_span(Span::empty(0)),
                            );
                        }
                    }
                }
                RootKind::Type => {
                    if let Some(v) = root
                        .value
                        .as_ref()
                        .and_then(|v| v.as_value())
                        .and_then(|v| v.as_string())
                    {
                        if let Some(existing) = file_type.replace(KnownRoot {
                            idx,
                            value: v.parse().unwrap(),
                        }) {
                            self.push(
                                D::ShadowedRoot {
                                    shadower: idx,
                                    shadowee: existing.idx,
                                }
                                .default_span(Span::empty(0)),
                            );
                        }
                    }
                }
                RootKind::Linked => {
                    let Some(value) = &root.value else {
                        continue;
                    };
                    match value {
                        RootValue::Value(Value::Container { items, .. }) => {
                            if let Some(existing) = linked.replace(KnownRoot {
                                idx,
                                value: items
                                    .iter()
                                    .filter_map(|v| {
                                        v.clone()
                                            .try_coerce_to(PropertyKind::String)
                                            .ok()
                                            .and_then(|v| v.into_string())
                                    })
                                    .collect(),
                            }) {
                                self.push(
                                    D::ShadowedRoot {
                                        shadower: idx,
                                        shadowee: existing.idx,
                                    }
                                    .default_span(Span::empty(0)),
                                );
                            }
                        }
                        _ => {
                            continue;
                        }
                    }
                }
                RootKind::Entries => match root.value.take() {
                    Some(RootValue::Value(Value::Map {
                        entries: map, span, ..
                    })) => {
                        let items = map
                            .into_iter()
                            .filter_map(|(k, v)| match (k, v) {
                                (Value::Hash(path_hash), Some(Value::Embedded(object))) => {
                                    Some(RootEntry { path_hash, object })
                                }
                                _ => None,
                            })
                            .collect();
                        root.value = Some(RootValue::Entries(Spanned::new(span, items)));
                        if let Some(shadowee) = entries.replace(idx) {
                            self.push(
                                D::ShadowedRoot {
                                    shadower: idx,
                                    shadowee,
                                }
                                .default_span(Span::empty(0)),
                            );
                        }
                    }
                    // not a well-formed map: leave the raw value in place to navigate/diagnose
                    other => root.value = other,
                },
            }
        }

        roots.file_type = file_type;
        roots.version = version;
        roots.linked = linked;
        roots.entries = entries;

        for kind in roots.missing() {
            self.push(D::MissingRootEntry { root_kind: kind }.default_span(Span::empty(0)));
        }

        Ast {
            roots,
            diagnostics: self.diagnostics,
        }
    }

    fn resolve_root(&mut self, node: &Node, idx: usize) -> Option<Root> {
        match node.kind {
            Kind::Comment | Kind::ErrorTree => return None,
            Kind::Entry => match self.resolve_entry(node, None, None) {
                Ok(entry) => {
                    let kind = RootKind::from_value(&entry.key);

                    return Some(Root {
                        idx,
                        name: kind.with_span(entry.key.span()),
                        type_expr: entry.type_expr,
                        value: entry.value.map(RootValue::Value),
                    });
                }
                Err(e) => {
                    self.push(e.fallback(node.span));
                }
            },
            _ => {
                self.push(D::RootNonEntry.default_span(node.span));
            }
        }
        None
    }
}
