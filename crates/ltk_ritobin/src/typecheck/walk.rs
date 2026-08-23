use ltk_meta::{property::values, traits::PropertyExt, PropertyKind, PropertyValueEnum};

use crate::{
    cst::{
        self,
        visitor::{Visit, VisitCtx},
        Kind, NodeId, Visitor,
    },
    parse::Span,
    typecheck::{
        coerce::CoerceFrom,
        diagnostics::{self, RitoTypeOrVirtual},
        ir::{IrEntry, IrItem, IrListItem},
    },
    PropertyValueExt as _, RitoType,
};

use super::{
    listlikes::try_populate_listlike,
    resolve::{resolve_entry, resolve_value},
    state::{RootEntry, RootKindOrUnknown, TypeChecker},
    trace::trace,
};

use diagnostics::Diagnostic::*;

impl<'a> TypeChecker<'a> {
    /// Reports whatever a container had to say about the value just pushed into it.
    ///
    /// - `span` - the value that was pushed, to underline
    /// - `expected_span` - where the container's type was written, so a rejection can point at it
    /// - `result` - what the push returned
    fn handle_container_res(
        &mut self,
        span: Span,
        expected_span: Option<Span>,
        result: Result<(), ltk_meta::Error>,
    ) {
        match result {
            Ok(()) => {}
            Err(ltk_meta::Error::MismatchedContainerTypes { expected, got }) => {
                self.ctx.diagnostics.push(
                    TypeMismatch {
                        span,
                        expected: RitoType::simple(expected),
                        expected_span,
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

    /// Reports a child whose shape its parent does not accept.
    ///
    /// - `child` - the offending item, underlined whole because that is what the parent rejected
    /// - `parent` - the type that rejected it, which also fixes the shape it wanted
    ///
    /// # Panics
    /// If `parent` has no body to hold items. Only the arms of [`Self::merge_ir`] that accept
    /// children reach this - anything else lands in its catch-all as an `UnexpectedContainerItem`.
    fn report_wrong_item_shape(&mut self, child: &IrItem, parent: RitoType) {
        let expected = parent
            .item_shape()
            .expect("only a parent with a body can reject an item shape");

        self.ctx.diagnostics.push(
            UnexpectedItem {
                span: child.span(),
                parent,
                expected,
            }
            .unwrap(),
        );
    }

    /// Reports an entry key that cannot become the key type its parent needs.
    ///
    /// - `span` - the key, to underline
    /// - `got` - the type the key resolved to
    /// - `expected` - the key type the parent needs
    /// - `expected_span` - where that type was written, or `None` when nothing wrote it - a
    ///   property name is a hash because it is a property name, not because of a type expression
    fn report_bad_entry_key(
        &mut self,
        span: Span,
        got: RitoType,
        expected: PropertyKind,
        expected_span: Option<Span>,
    ) {
        self.ctx.diagnostics.push(
            TypeMismatch {
                span,
                expected: RitoType::simple(expected),
                expected_span,
                got: got.into(),
            }
            .unwrap(),
        );
    }

    fn merge_ir(&mut self, mut parent: IrItem, child: IrItem) -> IrItem {
        let parent_type = parent.value().rito_type();
        let expected_span = parent.type_span();

        match &mut parent.value_mut() {
            PropertyValueEnum::Container(list)
            | PropertyValueEnum::UnorderedContainer(values::UnorderedContainer(list)) => {
                match child {
                    IrItem::ListItem(IrListItem(mut value)) => {
                        if value.kind() != list.item_kind() {
                            value = list.item_kind().coerce_from(value.clone()).unwrap_or(value);
                        }

                        let span = *value.meta();
                        let result = list.push(value);
                        self.handle_container_res(span, expected_span, result);
                    }
                    child @ IrItem::Entry(_) => {
                        self.report_wrong_item_shape(&child, parent_type);
                        return parent;
                    }
                }
            }
            PropertyValueEnum::Struct(struct_val)
            | PropertyValueEnum::Embedded(values::Embedded(struct_val)) => {
                let IrEntry { key, value, .. } = match child {
                    IrItem::Entry(entry) => entry,
                    child => {
                        self.report_wrong_item_shape(&child, parent_type);
                        return parent;
                    }
                };

                let (key_span, key_type) = (*key.meta(), key.rito_type());
                let Some(PropertyValueEnum::Hash(key)) = PropertyKind::Hash.coerce_from(key) else {
                    self.report_bad_entry_key(key_span, key_type, PropertyKind::Hash, None);
                    return parent;
                };

                struct_val.properties.insert(*key, value);
            }
            PropertyValueEnum::Map(map_value) => {
                let IrEntry { key, value, .. } = match child {
                    IrItem::Entry(entry) => entry,
                    child => {
                        self.report_wrong_item_shape(&child, parent_type);
                        return parent;
                    }
                };
                let span = *value.meta();
                let key_kind = map_value.key_kind();
                let (key_span, key_type) = (*key.meta(), key.rito_type());
                let Some(key) = key_kind.coerce_from(key) else {
                    self.report_bad_entry_key(key_span, key_type, key_kind, expected_span);
                    return parent;
                };
                let result = map_value.push(key, value);
                self.handle_container_res(span, expected_span, result);
            }
            PropertyValueEnum::Optional(option) => {
                let IrListItem(child) = match child {
                    IrItem::ListItem(item) => item,
                    child => {
                        self.report_wrong_item_shape(&child, parent_type);
                        return parent;
                    }
                };
                let child_span = *child.meta();
                let child_type = child.rito_type();
                let Some(child) = option.item_kind().coerce_from(child) else {
                    self.ctx.diagnostics.push(
                        TypeMismatch {
                            span: child_span,
                            expected: RitoType::simple(option.item_kind()),
                            expected_span,
                            got: child_type.into(),
                        }
                        .unwrap(),
                    );
                    return parent;
                };

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

                trace!("cant inject into {:?}", other.kind());
            }
        }
        parent
    }
}

impl Visitor for TypeChecker<'_> {
    fn enter_tree(&mut self, ctx: &VisitCtx, tree: NodeId) -> Visit {
        let tree = ctx.node(tree).unwrap();
        self.depth += 1;
        let depth = self.depth;

        self.trace_stack(depth, ">", tree.kind);

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

                        if matches!(value_type, K::Struct | K::Embedded) {
                            self.ctx.diagnostics.push(
                                MissingClassName {
                                    span: tree.open_brace_span(ctx.cst),
                                    expected: RitoType::simple(value_type),
                                }
                                .unwrap(),
                            );
                        }

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

                let type_span = parent.type_span();

                match resolve_value(&mut self.ctx, ctx, tree, value_hint, type_span) {
                    Ok(Some(item)) => {
                        trace!("  list item {item:?}");
                        if color_vec_type.is_some() {
                            self.list_queue.push(IrListItem(item));
                        } else {
                            self.stack.push((depth, IrItem::ListItem(IrListItem(item))));
                        }
                    }
                    Ok(None) => {
                        trace!("  ERROR empty item");
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
                                    expected_span: type_span,
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
                    parent.and_then(|p| p.1.type_span()),
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

        self.trace_stack(depth, "<", tree.kind);
        if tree.kind == cst::Kind::ErrorTree {
            return Visit::Continue;
        }

        if let Some(mut ir) = self.stack.pop() {
            self.trace_popped(depth, ir.0);
            if ir.0 != depth {
                self.stack.push(ir);
                return Visit::Continue;
            }

            // a listlike written as a list item declares no type of its own
            let type_span =
                ir.1.type_span()
                    .or_else(|| self.stack.last().and_then(|(_, parent)| parent.type_span()));

            if let Err(e) = try_populate_listlike(&mut ir.1, &mut self.list_queue, type_span) {
                self.ctx.diagnostics.push(e.fallback(*ir.1.value().meta()));
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
                        ..
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

        Visit::Continue
    }
}
