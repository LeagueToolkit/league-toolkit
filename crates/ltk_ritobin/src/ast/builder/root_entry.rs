use crate::{
    ast::{
        diagnostics::Diagnostic as D, node::TypeExpr, Ptr, Root, RootEntry, RootEntryKind,
        RootValue, Roots, Value,
    },
    cst::Kind,
    parse::Span,
    Node, RitoType, Spanned, SpannedExt,
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
#[derive(Debug, Clone)]
pub struct RawRootProperty {
    pub key: Spanned<RootEntryKind>,
    pub type_expr: Spanned<Option<TypeExpr>>,
    pub value: Option<Value>,
}

impl<'a> Builder<'a> {
    pub(crate) fn build_root(&mut self) -> Ast {
        let root_node = self.cst.root();

        let mut roots = Roots::new(root_node.children.get(self.cst).iter().filter_map(|child| {
            let node = child.tree(self.cst)?;
            self.resolve_root(node)
        }));

        for root in &roots {
            match *root.name {
                RootEntryKind::Unknown => {
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
                RootEntryKind::Version => {
                    let Some(value) = &root.value else {
                        continue;
                    };
                    match value.try_coerce_to(PropertyKind::U32) {}
                }
                RootEntryKind::Type => todo!(),
                RootEntryKind::Linked => todo!(),
                RootEntryKind::Entries => todo!(),
            }
        }

        todo!()
    }

    fn resolve_root(&mut self, node: &Node) -> Option<Root> {
        match node.kind {
            Kind::Comment | Kind::ErrorTree => return None,
            Kind::Entry => match self.resolve_entry(node, None, None) {
                Ok(entry) => {
                    let kind = RootEntryKind::from_value(&entry.key);

                    return Some(Root {
                        name: kind.with_span(entry.key.span()),
                        type_expr: entry.type_expr,
                        value: entry.value,
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

    // fn resolve_raw_root_property(&mut self, prop: RawRootProperty) -> Option<Root> {
    // let Some(value) = prop.value else {
    //     self.push(
    //         D::MissingEntryValue {
    //             key_span: prop.key.span,
    //             expected: prop
    //                 .key
    //                 .expected_type()
    //                 .map(|t| t.with_span(prop.type_expr.span)),
    //         }
    //         .unwrap(),
    //     );
    //     return None;
    // };
    // let Some(desired) = prop.key.expected_type() else {
    //     self.push(
    //         D::UnknownRoot {
    //             span: prop.key.span,
    //         }
    //         .unwrap(),
    //     );
    //     return None;
    // };
    //
    // Some(match *prop.key {
    //     // unknown has no expected type, so this arm is unreachable via the above UnknownRoot
    //     // return
    //     RootEntryKind::Unknown => unreachable!(),
    //     RootEntryKind::Version | RootEntryKind::Type => Root {
    //         key: prop.key,
    //         type_expr: prop.type_expr,
    //         value: Some(RootValue::Value(match value.coerce_to(desired.base) {
    //             Ok(v) => v,
    //             Err(v) => {
    //                 self.push(
    //                     D::TypeMismatch {
    //                         span: prop.key.span,
    //                         expected: desired.into(),
    //                         expected_span: Some(prop.type_expr.span),
    //                         got: v.rito_type().into(),
    //                     }
    //                     .unwrap(),
    //                 );
    //                 v
    //             }
    //         })),
    //     },
    //     RootEntryKind::Linked => Root {
    //         key: prop.key,
    //         type_expr: prop.type_expr,
    //         value: Some(match value {
    //             Value::Container {
    //                 item_kind,
    //                 items,
    //                 span,
    //             } if Some(item_kind) == desired.value_subtype() => {
    //                 RootValue::Dependencies(items.with_span(span))
    //             }
    //             value => {
    //                 self.push(
    //                     D::TypeMismatch {
    //                         span: prop.key.span,
    //                         expected: desired.into(),
    //                         expected_span: Some(prop.type_expr.span),
    //                         got: value.rito_type().into(),
    //                     }
    //                     .unwrap(),
    //                 );
    //                 RootValue::Value(value)
    //             }
    //         }),
    //     },
    //     RootEntryKind::Entries => Root {
    //         key: prop.key,
    //         type_expr: prop.type_expr,
    //         value: Some(match value {
    //             Value::Map {
    //                 key_kind,
    //                 value_kind,
    //                 entries,
    //                 span,
    //             } if key_kind == desired.subtype(0) && value_kind == desired.subtype(1) => {
    //                 RootValue::Entries(
    //                     entries
    //                         .into_iter()
    //                         .filter_map(|(k, v)| {
    //                             Some(RootEntry {
    //                                 object: match v {
    //                                     Some(v) => match v.coerce_to(value_kind) {
    //                                         Ok(Value::Embedded(object)) => object,
    //                                         Ok(v) | Err(v) => {
    //                                             self.push(
    //                                                 D::TypeMismatch {
    //                                                     span: prop.key.span,
    //                                                     expected: RitoType::simple(value_kind)
    //                                                         .into(),
    //                                                     expected_span: Some(
    //                                                         prop.type_expr.span,
    //                                                     ),
    //                                                     got: v.rito_type().into(),
    //                                                 }
    //                                                 .unwrap(),
    //                                             );
    //                                             return None;
    //                                         }
    //                                     },
    //                                     None => {
    //                                         self.push(
    //                                             D::MissingEntryValue {
    //                                                 key_span: k.span(),
    //                                                 expected: Some(
    //                                                     RitoType::simple(value_kind)
    //                                                         .with_span(prop.type_expr.span),
    //                                                 ),
    //                                             }
    //                                             .unwrap(),
    //                                         );
    //                                         return None;
    //                                     }
    //                                 },
    //                                 path_hash: match k.coerce_to(key_kind) {
    //                                     Ok(Value::Hash(hash)) => hash,
    //                                     Ok(k) | Err(k) => {
    //                                         self.push(
    //                                             D::TypeMismatch {
    //                                                 span: prop.key.span,
    //                                                 expected: RitoType::simple(key_kind).into(),
    //                                                 expected_span: Some(prop.type_expr.span),
    //                                                 got: k.rito_type().into(),
    //                                             }
    //                                             .unwrap(),
    //                                         );
    //                                         return None;
    //                                     }
    //                                 },
    //                             })
    //                         })
    //                         .collect::<Vec<_>>()
    //                         .with_span(span),
    //                 )
    //             }
    //             value => {
    //                 self.push(
    //                     D::TypeMismatch {
    //                         span: prop.key.span,
    //                         expected: desired.into(),
    //                         expected_span: Some(prop.type_expr.span),
    //                         got: value.rito_type().into(),
    //                     }
    //                     .unwrap(),
    //                 );
    //                 RootValue::Value(value)
    //             }
    //         }),
    //     },
    // })
    // }

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
                                object: s,
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

        todo!()
        // Ast {
        //     bin_type,
        //     version,
        //     dependencies,
        //     objects,
        //     diagnostics: std::mem::take(&mut self.diagnostics),
        // }
    }
}
