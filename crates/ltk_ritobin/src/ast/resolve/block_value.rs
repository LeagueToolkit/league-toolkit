use ltk_meta::PropertyKind;

use crate::{
    ast::{
        builder::Builder,
        diagnostics::{Diagnostic::*, MaybeSpanDiag},
        Property, Value,
    },
    cst::Kind,
    parse::Span,
    Node, RitoType,
};

impl<'a> Builder<'a> {
    /// Attempt to resolve a `Block`/`ListItemBlock` node to a value
    pub(crate) fn resolve_block_value(
        &mut self,
        block: &Node,
        hint: RitoType,
        hint_span: Option<Span>,
    ) -> Result<Value, MaybeSpanDiag> {
        use PropertyKind as K;

        match hint.base {
            K::Struct | K::Embedded => {
                self.push(
                    MissingClassName {
                        span: block.open_brace_span(self.cst),
                        expected: hint,
                    }
                    .unwrap(),
                );

                Ok(Value::Unresolved {
                    span: block.span,
                    kind: hint.base,
                })
            }
            K::Vector2 | K::Vector3 | K::Vector4 | K::Color | K::Matrix44 => {
                Ok(self.resolve_listlike(block, hint.base, hint_span))
            }
            K::Map => {
                let key_kind = hint.subtype(0);
                let value_kind = hint.subtype(1);
                let entries = self.resolve_body_map_entries(block, key_kind, value_kind, hint_span);
                Ok(Value::Map {
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
                    K::Container => Value::Container {
                        item_kind,
                        items,
                        span: block.span,
                    },
                    K::UnorderedContainer => Value::UnorderedContainer {
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
                    let content: Vec<&Node> = block
                        .children
                        .get(self.cst)
                        .iter()
                        .filter_map(|c| c.tree(self.cst))
                        .filter(|n| n.kind != Kind::Comment)
                        .collect();

                    // `option[vec3] = { { 0.5, 5.3, -0.2 } }` - the listlike wrapped in its own
                    // block, the same shape a listlike takes as a `list[vec3]` item.
                    if let [only] = content[..] {
                        if only.kind == Kind::ListItemBlock {
                            let inner = self.resolve_list_item_block(only, item_hint, hint_span)?;
                            return Ok(Value::Optional {
                                item_kind: Some(item_kind),
                                value: Some(Box::new(inner)),
                                span: block.span,
                            });
                        }
                    }

                    if content.is_empty() {
                        return Ok(Value::Optional {
                            item_kind: Some(item_kind),
                            value: None,
                            span: block.span,
                        });
                    }
                    // an optional listlike spells its components flat, same as a bare listlike
                    let inner = self.resolve_listlike(block, item_kind, hint_span);
                    return Ok(Value::Optional {
                        item_kind: Some(item_kind),
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
                                Ok(v) => match v.try_coerce_to(item_kind) {
                                    Ok(coerced) => value = Some(coerced),
                                    Err(v) => self.push(
                                        TypeMismatch {
                                            span: v.span(),
                                            expected: RitoType::simple(item_kind).into(),
                                            expected_span: hint_span,
                                            got: v.rito_type().into(),
                                        }
                                        .unwrap(),
                                    ),
                                },
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
                                span: node.trimmed_span(self.cst),
                                parent: hint,
                                expected: crate::ItemShape::Value,
                            }
                            .unwrap(),
                        ),
                    }
                }
                Ok(Value::Optional {
                    item_kind: Some(item_kind),
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

    pub(crate) fn resolve_body_properties(
        &mut self,
        block: &Node,
        hint: RitoType,
    ) -> Vec<Property> {
        let mut properties = Vec::new();
        for child in block.children.get(self.cst).iter() {
            let Some(node) = child.tree(self.cst) else {
                continue;
            };
            match node.kind {
                Kind::Comment => continue,
                Kind::Entry => match self.resolve_entry(node, Some(hint), None) {
                    Ok(entry) => match entry.key.try_coerce_to(PropertyKind::Hash) {
                        Ok(Value::Hash(hash)) => properties.push(Property {
                            name: hash,
                            type_expr: entry.type_expr,
                            value: entry.value,
                        }),
                        Ok(value) | Err(value) => self.push(
                            TypeMismatch {
                                span: value.span(),
                                expected: RitoType::simple(PropertyKind::Hash).into(),
                                expected_span: None,
                                got: value.rito_type().into(),
                            }
                            .unwrap(),
                        ),
                    },
                    Err(e) => self.push(e.fallback(node.span)),
                },
                Kind::ListItem | Kind::ListItemBlock => self.push(
                    UnexpectedItem {
                        span: node.trimmed_span(self.cst),
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
    ) -> Vec<(Value, Option<Value>)> {
        let hint = RitoType::map(key_kind, value_kind);
        let mut entries = Vec::new();
        for child in block.children.get(self.cst).iter() {
            let Some(node) = child.tree(self.cst) else {
                continue;
            };
            match node.kind {
                Kind::Comment => continue,
                Kind::Entry => match self.resolve_entry(node, Some(hint), hint_span) {
                    Ok(entry) => match entry.key.try_coerce_to(key_kind) {
                        Ok(key) => {
                            match entry.value.as_ref() {
                                Some(value) if value.kind().is_some_and(|k| k != value_kind) => {
                                    self.push(
                                        TypeMismatch {
                                            span: value.span(),
                                            expected: RitoType::simple(value_kind).into(),
                                            expected_span: hint_span,
                                            got: value.rito_type().into(),
                                        }
                                        .unwrap(),
                                    );
                                }
                                _ => {
                                    // reporting the error for not having a value should be handled already
                                }
                            }
                            entries.push((key, entry.value));
                        }
                        Err(key) => self.push(
                            TypeMismatch {
                                span: key.span(),
                                expected: RitoType::simple(key_kind).into(),
                                expected_span: hint_span,
                                got: key.rito_type().into(),
                            }
                            .unwrap(),
                        ),
                    },
                    Err(e) => self.push(e.fallback(node.span)),
                },
                Kind::ListItem | Kind::ListItemBlock => self.push(
                    UnexpectedItem {
                        span: node.trimmed_span(self.cst),
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
    ) -> Vec<Value> {
        let item_hint = RitoType::simple(item_kind);
        let mut items = Vec::new();
        for child in block.children.get(self.cst).iter() {
            let Some(node) = child.tree(self.cst) else {
                continue;
            };
            match node.kind {
                Kind::Comment => continue,
                Kind::ListItem => {
                    match self
                        .resolve_value(node, Some(item_hint), hint_span)
                        .and_then(|value| {
                            value
                                .try_coerce_to(item_kind)
                                .map_err(|value| TypeMismatch {
                                    span: value.span(),
                                    expected: RitoType::simple(item_kind).into(),
                                    expected_span: hint_span,
                                    got: value.rito_type().into(),
                                })
                        }) {
                        Ok(value) => {
                            items.push(value);
                        }
                        Err(e) => self.push(e.default_span(node.span)),
                    }
                }
                Kind::ListItemBlock => {
                    match self.resolve_list_item_block(node, item_hint, hint_span) {
                        Ok(v) => items.push(v),
                        Err(e) => self.push(e.fallback(node.span)),
                    }
                }
                Kind::Entry => self.push(
                    UnexpectedItem {
                        span: node.trimmed_span(self.cst),
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
}
