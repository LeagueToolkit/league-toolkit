use ltk_meta::{property::values, traits::PropertyExt, PropertyKind, PropertyValueEnum};

use crate::{
    cst::{
        self,
        visitor::{Visit, VisitCtx},
        Kind, NodeId, Visitor,
    },
    parse::Span,
    typecheck::{
        diagnostics::{self, RitoTypeOrVirtual},
        ir::{IrEntry, IrItem, IrListItem},
    },
    PropertyValueExt as _, RitoType,
};

use super::{
    resolve::{coerce_type, resolve_entry, resolve_value},
    state::{RootEntry, RootKindOrUnknown, TypeChecker},
    trace::trace,
    vecmath::populate_vec_or_color,
};

use diagnostics::Diagnostic::*;

impl<'a> TypeChecker<'a> {
    fn handle_container_res(&mut self, span: Span, result: Result<(), ltk_meta::Error>) {
        match result {
            Ok(()) => {}
            Err(ltk_meta::Error::MismatchedContainerTypes { expected, got }) => {
                self.ctx.diagnostics.push(
                    TypeMismatch {
                        span,
                        expected: RitoType::simple(expected),
                        expected_span: None, // TODO: would be nice here
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

    fn merge_ir(&mut self, mut parent: IrItem, child: IrItem) -> IrItem {
        match &mut parent.value_mut() {
            PropertyValueEnum::Container(list)
            | PropertyValueEnum::UnorderedContainer(values::UnorderedContainer(list)) => {
                match child {
                    IrItem::ListItem(IrListItem(mut value)) => {
                        if value.kind() != list.item_kind() {
                            value = coerce_type(value.clone(), list.item_kind()).unwrap_or(value);
                        }

                        let span = *value.meta();
                        let result = list.push(value);
                        self.handle_container_res(span, result);
                    }
                    IrItem::Entry(IrEntry { key: _, value: _ }) => {
                        trace!("\x1b[41mlist item must be list item\x1b[0m");
                        return parent;
                    }
                }
            }
            PropertyValueEnum::Struct(struct_val)
            | PropertyValueEnum::Embedded(values::Embedded(struct_val)) => {
                let IrItem::Entry(IrEntry { key, value }) = child else {
                    trace!("\x1b[41mstruct item must be entry\x1b[0m");
                    return parent;
                };

                let Some(PropertyValueEnum::Hash(key)) = coerce_type(key, PropertyKind::Hash)
                else {
                    return parent;
                };

                struct_val.properties.insert(*key, value);
            }
            PropertyValueEnum::Map(map_value) => {
                let IrItem::Entry(IrEntry { key, value }) = child else {
                    trace!("map item must be entry");
                    return parent;
                };
                let span = *value.meta();
                let Some(key) = coerce_type(key, map_value.key_kind()) else {
                    return parent;
                };
                let result = map_value.push(key, value);
                self.handle_container_res(span, result);
            }
            PropertyValueEnum::Optional(option) => {
                let IrItem::ListItem(IrListItem(child)) = child else {
                    trace!("\x1b[41moptional value must be list item\x1b[0m");
                    return parent;
                };
                if child.kind() != option.item_kind() {
                    self.ctx.diagnostics.push(
                        TypeMismatch {
                            span: *child.meta(),
                            expected: RitoType::simple(option.item_kind()),
                            expected_span: None, // TODO: would be nice here
                            got: child.rito_type().into(),
                        }
                        .unwrap(),
                    );
                    return parent;
                }

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

                match resolve_value(&mut self.ctx, ctx, tree, value_hint) {
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
                                    expected_span: None,
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

        match self.stack.pop() {
            Some(mut ir) => {
                self.trace_popped(depth, ir.0);
                if ir.0 != depth {
                    self.stack.push(ir);
                    return Visit::Continue;
                }

                if !self.list_queue.is_empty() {
                    if let Err(e) = populate_vec_or_color(&mut ir.1, &mut self.list_queue) {
                        self.ctx.diagnostics.push(e.fallback(*ir.1.value().meta()));
                    }
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
            _ => {}
        }

        Visit::Continue
    }
}
