use ltk_meta::PropertyKind;

use crate::{
    ast::{
        builder::Builder,
        diagnostics::{Diagnostic as D, MaybeSpanDiag, TypeMismatch},
        node::{Map, MapEntry},
        resolve::literals::ExpectedType,
        Property, Value,
    },
    cst::Kind,
    ContainerType, Node, RitoType, ScalarType,
};
use crate::{rito, span::Span};

impl<'a> Builder<'a> {
    /// Attempt to resolve a `Block`/`ListItemBlock` node to a value
    pub(crate) fn resolve_block_value(
        &mut self,
        block: &Node,
        hint: impl Into<RitoType>,
        hint_span: Option<Span>,
    ) -> Result<Value, MaybeSpanDiag> {
        use ContainerType as C;
        use PropertyKind as K;
        use ScalarType as S;

        let hint = hint.into();
        match hint {
            RitoType::Scalar(S::Struct | S::Embedded) => {
                self.push(
                    D::MissingClassName {
                        span: block.open_brace_span(self.cst),
                        expected: hint,
                    }
                    .unwrap(),
                );

                Ok(Value::Unresolved {
                    span: block.span,
                    kind: hint,
                })
            }
            RitoType::Scalar(
                hint @ (S::Vector2 | S::Vector3 | S::Vector4 | S::Color | S::Matrix44),
            ) => Ok(self.resolve_listlike(block, hint, hint_span)),
            RitoType::Container(C::Map { key, value }) => {
                let key_kind = key.unwrap_or_default();
                let value_kind = value.unwrap_or_default();
                let entries = self.resolve_body_map_entries(block, hint, hint_span);
                Ok(Map {
                    key_kind,
                    value_kind,
                    entries,
                    span: block.span,
                }
                .into())
            }
            RitoType::Container(C::Container(item_kind) | C::UnorderedContainer(item_kind)) => {
                let item_kind = item_kind.unwrap_or_default();
                let items = self.resolve_body_items(block, hint, hint_span);
                Ok(match hint.base() {
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
            RitoType::Container(C::Optional(item_kind)) => {
                let item_kind = item_kind.unwrap_or_default();
                if matches!(
                    item_kind.kind(),
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
                            let inner = self.resolve_list_item_block(only, item_kind, hint_span)?;
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
                    let inner = self.resolve_listlike(block, item_kind.into(), hint_span);
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
                        Kind::Comment | Kind::Annotation => continue,
                        Kind::ListItem => {
                            match self.resolve_value(
                                node,
                                Some(ExpectedType::from(item_kind)),
                                hint_span,
                            ) {
                                Ok(v) => match v.try_coerce_to(item_kind) {
                                    Ok(coerced) => value = Some(coerced),
                                    Err(v) => self.push(D::unwrap(
                                        TypeMismatch {
                                            span: v.span(),
                                            expected: item_kind.into(),
                                            expected_span: hint_span,
                                            got: v.rito_type().into(),
                                        }
                                        .into(),
                                    )),
                                },
                                Err(e) => self.push(e.default_span(node.span)),
                            }
                        }
                        Kind::ListItemBlock => {
                            match self.resolve_list_item_block(node, item_kind, hint_span) {
                                Ok(v) => value = Some(v),
                                Err(e) => self.push(e.fallback(node.span)),
                            }
                        }
                        _ => self.push(
                            D::UnexpectedItem {
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
            _ => Err(D::UnexpectedContainerItem {
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
                Kind::Comment | Kind::Annotation => continue,
                Kind::Entry => match self.resolve_entry(node, Some(hint), None) {
                    Ok(entry) => match entry.key.try_coerce_to_bin_hash() {
                        Ok(hash) => properties.push(Property {
                            name: hash,
                            type_expr: entry.type_expr,
                            value: entry.value,
                        }),
                        Err(value) => self.push(D::unwrap(
                            TypeMismatch {
                                span: value.span(),
                                expected: rito!(Hash).into(),
                                expected_span: None,
                                got: value.rito_type().into(),
                            }
                            .into(),
                        )),
                    },
                    Err(e) => self.push(e.fallback(node.span)),
                },
                _ => {
                    self.push(
                        D::UnexpectedItem {
                            span: node.trimmed_span(self.cst),
                            parent: hint,
                            expected: crate::ItemShape::Entry,
                        }
                        .unwrap(),
                    );
                }
            }
        }
        properties
    }

    fn resolve_body_map_entries(
        &mut self,
        block: &Node,
        hint: RitoType,
        hint_span: Option<Span>,
    ) -> Vec<MapEntry> {
        let key_hint = hint.subtype(0).unwrap_or_default();
        let value_kind = hint.subtype(1).unwrap_or_default();
        let mut entries = Vec::new();
        for child in block.children.get(self.cst).iter() {
            let Some(node) = child.tree(self.cst) else {
                continue;
            };
            match node.kind {
                Kind::Comment | Kind::Annotation => continue,
                Kind::Entry => match self.resolve_entry(node, Some(hint), hint_span) {
                    Ok(entry) => match entry.key.try_coerce_to(key_hint) {
                        Ok(key) => {
                            match entry.value.as_ref() {
                                Some(value)
                                    if value.rito_type().is_some_and(|k| {
                                        k.with_default_hash_types()
                                            != value_kind.with_default_hash_type()
                                    }) =>
                                {
                                    self.push(D::unwrap(
                                        TypeMismatch {
                                            span: value.span(),
                                            expected: hint.subtype(1).into(),
                                            expected_span: hint_span,
                                            got: value.rito_type().into(),
                                        }
                                        .into(),
                                    ));
                                }
                                _ => {
                                    // reporting the error for not having a value should be handled already
                                }
                            }
                            entries.push(MapEntry {
                                key,
                                value: entry.value,
                            });
                        }
                        Err(key) => self.push(D::unwrap(
                            TypeMismatch {
                                span: key.span(),
                                expected: key_hint.into(),
                                expected_span: hint_span,
                                got: key.rito_type().into(),
                            }
                            .into(),
                        )),
                    },
                    Err(e) => self.push(e.fallback(node.span)),
                },
                _ => {
                    self.push(
                        D::UnexpectedItem {
                            span: node.trimmed_span(self.cst),
                            parent: hint,
                            expected: crate::ItemShape::Entry,
                        }
                        .unwrap(),
                    );
                }
            }
        }
        entries
    }

    fn resolve_body_items(
        &mut self,
        block: &Node,
        hint: RitoType,
        hint_span: Option<Span>,
    ) -> Vec<Value> {
        let item_kind = hint.subtype(0).unwrap_or_default();
        let mut items = Vec::new();
        for child in block.children.get(self.cst).iter() {
            let Some(node) = child.tree(self.cst) else {
                continue;
            };
            match node.kind {
                Kind::Comment | Kind::Annotation => continue,
                Kind::ListItem => {
                    match self
                        .resolve_value(node, Some(ExpectedType::from(item_kind)), hint_span)
                        .and_then(|value| {
                            value.try_coerce_to(item_kind).map_err(|value| {
                                TypeMismatch {
                                    span: value.span(),
                                    expected: item_kind.into(),
                                    expected_span: hint_span,
                                    got: value.rito_type().into(),
                                }
                                .into()
                            })
                        }) {
                        Ok(value) => {
                            items.push(value);
                        }
                        Err(e) => self.push(e.default_span(node.span)),
                    }
                }
                Kind::ListItemBlock => {
                    match self.resolve_list_item_block(node, item_kind, hint_span) {
                        Ok(v) => items.push(v),
                        Err(e) => self.push(e.fallback(node.span)),
                    }
                }
                _ => {
                    self.push(
                        D::UnexpectedItem {
                            span: node.trimmed_span(self.cst),
                            parent: hint,
                            expected: crate::ItemShape::Value,
                        }
                        .unwrap(),
                    );
                }
            }
        }
        items
    }
}
