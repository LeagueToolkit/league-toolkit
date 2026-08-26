use ltk_meta::{traits::PropertyExt as _, PropertyKind, PropertyValueEnum};

use crate::{
    ast::{
        build::BuildCtx,
        diagnostics::{Diagnostic::*, MaybeSpanDiag},
        AstProperty, AstValue, Spanned,
    },
    cst::Kind,
    parse::Span,
    Node, PropertyValueExt as _, RitoType,
};

impl<'a> BuildCtx<'a> {
    /// Attempt to resolve a `Block`/`ListItemBlock` node to a value
    pub(crate) fn resolve_block_value(
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
                            return Ok(AstValue::Optional {
                                item_kind,
                                value: Some(Box::new(inner)),
                                span: block.span,
                            });
                        }
                    }

                    if content.is_empty() {
                        return Ok(AstValue::Optional {
                            item_kind,
                            value: None,
                            span: block.span,
                        });
                    }
                    // an optional listlike spells its components flat, same as a bare listlike
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
                                Ok(Some(v)) if v.kind() == item_kind => value = Some(v),
                                Ok(Some(v)) => match v.clone().coerce_to(item_kind) {
                                    Some(coerced) => value = Some(coerced),
                                    None => self.push(
                                        TypeMismatch {
                                            span: v.span(),
                                            expected: RitoType::simple(item_kind),
                                            expected_span: hint_span,
                                            got: v.rito_type().into(),
                                        }
                                        .unwrap(),
                                    ),
                                },
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
                                span: node.trimmed_span(self.cst),
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

    pub(crate) fn resolve_body_properties(
        &mut self,
        block: &Node,
        hint: RitoType,
    ) -> Vec<AstProperty> {
        let mut properties = Vec::new();
        for child in block.children.get(self.cst).iter() {
            let Some(node) = child.tree(self.cst) else {
                continue;
            };
            match node.kind {
                Kind::Comment => continue,
                Kind::Entry => match self.resolve_entry(node, Some(hint), None) {
                    Ok(entry) => {
                        let key_span = entry.key.span();
                        let key_kind = entry.key.rito_type();
                        match entry.key.coerce_to(PropertyKind::Hash) {
                            Some(AstValue::Hash(hash)) => properties.push(AstProperty {
                                name: hash,
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
                        let key_span = entry.key.span();
                        let got_key_kind = entry.key.rito_type();
                        match entry.key.coerce_to(key_kind) {
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
                            match v.clone().coerce_to(item_kind) {
                                Some(coerced) => v = coerced,
                                None => {
                                    self.push(
                                        TypeMismatch {
                                            span: v.span(),
                                            expected: RitoType::simple(item_kind),
                                            expected_span: hint_span,
                                            got: v.rito_type().into(),
                                        }
                                        .unwrap(),
                                    );
                                    continue;
                                }
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
