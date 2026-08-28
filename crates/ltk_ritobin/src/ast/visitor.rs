use std::ops::ControlFlow::{self};

use crate::ast::{Ast, Object, Property, RootEntry, Value};

#[allow(unused_variables)]
/// [Visitor pattern](https://rust-unofficial.github.io/patterns/patterns/behavioural/visitor.html)
/// for walking an [`Ast`].
///
/// Every AST node has a matching `enter_*`/`exit_*` pair, which are called before/after walking a
/// node & it's children.
///
/// `enter_*` methods return an [`EnterFlow`], choosing whether to [`Descend`] into the node's
/// children. `exit_*` methods return an [`ExitFlow`], choosing whether to [`Continue`] with the
/// node's remaining siblings. Both share [`Break`], to stop or abort the walk early.
pub trait Visitor {
    /// Called before walking a [`RootEntry`]'s object (see [`Self::enter_object`]).
    fn enter_root_entry(&mut self, object: &RootEntry) -> EnterFlow {
        Descend::Children.into()
    }
    /// Called after a [`RootEntry`] has been walked.
    fn exit_root_entry(&mut self, object: &RootEntry) -> ExitFlow {
        Continue::Siblings.into()
    }

    /// Called before walking an [`Object`]'s properties.
    /// (see [`Self::enter_property`]).
    fn enter_object(&mut self, object: &Object) -> EnterFlow {
        Descend::Children.into()
    }
    /// Called after an [`Object`] has been walked.
    fn exit_object(&mut self, object: &Object) -> ExitFlow {
        Continue::Siblings.into()
    }

    /// Called before walking a [`Property`]'s children (its value - see [`Self::enter_value`]).
    fn enter_property(&mut self, property: &Property) -> EnterFlow {
        Descend::Children.into()
    }
    /// Called after a property's value has been walked.
    fn exit_property(&mut self, property: &Property) -> ExitFlow {
        Continue::Siblings.into()
    }

    /// Called before walking a [`Value`]'s children (if it has any).
    fn enter_value(&mut self, value: &Value) -> EnterFlow {
        Descend::Children.into()
    }

    /// Called after a value has been walked.
    fn exit_value(&mut self, value: &Value) -> ExitFlow {
        Continue::Siblings.into()
    }
}

pub trait VisitorExt: Sized + Visitor {
    fn walk(mut self, ast: &Ast) -> Self {
        ast.walk(&mut self);
        self
    }
}

impl<T: Sized + Visitor> VisitorExt for T {}

pub enum Break {
    /// Stop the walk. The matching exit callback still runs for every open node, bottom-up.
    Stop,
    /// Abort the walk immediately. No further callbacks run.
    Abort,
}

/// The continuation returned by `exit_*` methods.
pub enum Continue {
    /// With the parent's remaining children.
    Siblings,
    /// At the parent's exit callback: this node's exit returned [`Continue::Parent`], pruning the
    /// remaining siblings.
    Parent,
}

/// The continuation returned by `enter_*` methods.
pub enum Descend {
    /// Descend into this node's children.
    Children,
    /// Skip this node's children; its `exit_*` still runs.
    Skip,
}

/// Returned by `enter_*` [`Visitor`] methods.
pub type EnterFlow = ControlFlow<Break, Descend>;
/// Returned by `exit_*` [`Visitor`] methods.
pub type ExitFlow = ControlFlow<Break, Continue>;

impl From<Descend> for EnterFlow {
    fn from(descend: Descend) -> Self {
        ControlFlow::Continue(descend)
    }
}

impl From<Continue> for ExitFlow {
    fn from(cont: Continue) -> Self {
        ControlFlow::Continue(cont)
    }
}

fn walk_inner<V, T>(
    visitor: &mut V,
    node: &T,
    enter: fn(&mut V, &T) -> EnterFlow,
    exit: fn(&mut V, &T) -> ExitFlow,
    children: impl FnOnce(&mut V) -> ControlFlow<Break>,
) -> ExitFlow {
    let walked = match enter(visitor, node) {
        ControlFlow::Break(b) => ControlFlow::Break(b),
        ControlFlow::Continue(Descend::Skip) => ControlFlow::Continue(()),
        ControlFlow::Continue(Descend::Children) => children(visitor),
    };

    // an abort skips the remaining exits entirely
    if let ControlFlow::Break(Break::Abort) = walked {
        return ControlFlow::Break(Break::Abort);
    }

    match (walked, exit(visitor, node)) {
        (_, ControlFlow::Break(Break::Abort)) => ControlFlow::Break(Break::Abort),
        (ControlFlow::Break(Break::Stop), _) | (_, ControlFlow::Break(Break::Stop)) => {
            ControlFlow::Break(Break::Stop)
        }
        (_, exit_result) => exit_result,
    }
}

/// Walks `items`, stopping the loop early on [`Continue::Parent`] and propagating any [`Break`].
fn walk_all<V, T>(
    visitor: &mut V,
    items: impl IntoIterator<Item = T>,
    walk: fn(&mut V, T) -> ExitFlow,
) -> ControlFlow<Break> {
    for item in items {
        match walk(visitor, item)? {
            Continue::Siblings => {}
            Continue::Parent => break,
        }
    }
    ControlFlow::Continue(())
}

fn walk_root_object<V: Visitor>(visitor: &mut V, object: &RootEntry) -> ExitFlow {
    walk_inner(
        visitor,
        object,
        V::enter_root_entry,
        V::exit_root_entry,
        |v| walk_object(v, &object.object).map_continue(|_| ()),
    )
}

fn walk_object<V: Visitor>(visitor: &mut V, s: &Object) -> ExitFlow {
    walk_inner(visitor, s, V::enter_object, V::exit_object, |v| {
        walk_all(v, &s.properties, walk_property)
    })
}

fn walk_property<V: Visitor>(visitor: &mut V, property: &Property) -> ExitFlow {
    walk_inner(
        visitor,
        property,
        V::enter_property,
        V::exit_property,
        |v| {
            property
                .value
                .as_ref()
                .map(|value| walk_value(v, value).map_continue(|_| ()))
                .unwrap_or(ControlFlow::Continue(()))
        },
    )
}

fn walk_value<V: Visitor>(visitor: &mut V, value: &Value) -> ExitFlow {
    walk_inner(
        visitor,
        value,
        V::enter_value,
        V::exit_value,
        |v| match value {
            Value::Struct(s) | Value::Embedded(s) => walk_object(v, s).map_continue(|_| ()),
            Value::Container { items, .. } | Value::UnorderedContainer { items, .. } => {
                walk_all(v, items, walk_value)
            }
            Value::Map { entries, .. } => walk_all(v, entries, |v, (key, value)| {
                match walk_value(v, key)? {
                    Continue::Siblings => {}
                    // the key's exit pruned the entry's remaining sibling (its value) and,
                    // transitively, the rest of the map's entries
                    Continue::Parent => return Continue::Parent.into(),
                }
                match value {
                    Some(value) => walk_value(v, value),
                    None => Continue::Siblings.into(),
                }
            }),
            Value::Optional {
                value: Some(inner), ..
            } => walk_value(v, inner).map_continue(|_| ()),
            _ => ControlFlow::Continue(()),
        },
    )
}

impl Ast {
    /// Walk a [`Visitor`] over every object in this tree.
    pub fn walk<V: Visitor>(&self, visitor: &mut V) {
        let _ = walk_all(visitor, &self.objects, walk_root_object);
    }
}
