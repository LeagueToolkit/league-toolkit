use std::ops::ControlFlow::{self, Break, Continue};

use crate::ast::{Ast, Object, Property, RootObject, Value};

pub use crate::cst::visitor::Visit;

#[allow(unused_variables)]
/// [Visitor pattern](https://rust-unofficial.github.io/patterns/patterns/behavioural/visitor.html)
/// for walking an [`Ast`].
///
/// Every AST node has a matching `enter_*`/`exit_*` pair, which are called before/after walking a
/// node & it's children.
///
/// Each method returns a [`Visit`] result, which WIP WIP WIP
///
/// [`Visit::Skip`] behaviour depends on which visit method returns it:
/// - from `enter_*`: this node's children are not visited, but its `exit_*` still runs.
/// - from `exit_*`: the walk skips the *parent's* remaining children, and continues from the parent's `exit_*`.
pub trait Visitor {
    /// Called before walking a [`RootObject`]'s object (see [`Self::enter_object`]).
    fn enter_root_object(&mut self, object: &RootObject) -> Visit {
        Visit::Continue
    }
    /// Called after a root object has been walked, or would have been but was skipped.
    ///
    /// A [`Visit::Skip`] here prunes the remaining sibling objects: the walk ends without
    /// visiting any objects after this one.
    fn exit_root_object(&mut self, object: &RootObject) -> Visit {
        Visit::Continue
    }

    /// Called before walking an [`Object`]'s properties.
    /// (see [`Self::enter_property`]).
    fn enter_object(&mut self, object: &Object) -> Visit {
        Visit::Continue
    }
    /// Called after an [`Object`] has been walked,
    /// or when unwinding due to a child returning [`Visit::Skip`]/[`Visit::Stop`].
    ///
    /// Returning [`Visit::Skip`] skips any remaining sibling *properties* for this struct.
    fn exit_object(&mut self, object: &Object) -> Visit {
        Visit::Continue
    }

    /// Called before walking a [`Property`]'s children (its value - see [`Self::enter_value`]).
    fn enter_property(&mut self, property: &Property) -> Visit {
        Visit::Continue
    }
    /// Called after a property's value has been walked,
    /// or when unwinding due to a child returning [`Visit::Skip`]/[`Visit::Stop`].
    ///
    /// Returning [`Visit::Skip`] here skips this property's remaining sibling properties.
    fn exit_property(&mut self, property: &Property) -> Visit {
        Visit::Continue
    }

    /// Called before walking a [`Value`]'s children (if it has any).
    ///
    /// Returning [`Visit::Skip`] skips this value's children.
    fn enter_value(&mut self, value: &Value) -> Visit {
        Visit::Continue
    }
    /// Called after a value has been walked, or would have been but were skipped.
    ///
    /// A [`Visit::Skip`] here prunes whatever remains at this value's level: the rest of a
    /// container's items, the rest of a map's entries (a map entry's value is itself skippable
    /// this way - it prunes the remaining entries), or for a property's value or an object's
    /// struct, which have no siblings - nothing observable at all.
    fn exit_value(&mut self, value: &Value) -> Visit {
        Visit::Continue
    }
}

pub trait VisitorExt: Sized + Visitor {
    fn walk(mut self, ast: &Ast) -> Self {
        ast.walk(&mut self);
        self
    }
}

impl<T: Sized + Visitor> VisitorExt for T {}

enum Interrupt {
    /// A [`Visit::Stop`]: the matching exit callback still runs for every open node, bottom-up.
    Unwind,
    /// A [`Visit::Abort`]: no further callbacks run.
    Abort,
}

enum Resume {
    /// With the parent's remaining children.
    Siblings,
    /// At the parent's exit callback: the child's exit returned [`Visit::Skip`], pruning the
    /// remaining siblings.
    Parent,
}

type Walk = ControlFlow<Interrupt, Resume>;

fn walk_inner<V, T>(
    visitor: &mut V,
    node: &T,
    enter: fn(&mut V, &T) -> Visit,
    exit: fn(&mut V, &T) -> Visit,
    children: impl FnOnce(&mut V) -> ControlFlow<Interrupt>,
) -> Walk {
    let walked = match enter(visitor, node) {
        Visit::Abort => Break(Interrupt::Abort),
        Visit::Stop => Break(Interrupt::Unwind),
        Visit::Skip => Continue(()),
        Visit::Continue => children(visitor),
    };

    // an abort skips the remaining exits entirely
    if let Break(Interrupt::Abort) = walked {
        return Break(Interrupt::Abort);
    }

    match (walked, exit(visitor, node)) {
        (_, Visit::Abort) => Break(Interrupt::Abort),
        (Break(Interrupt::Unwind), _) | (_, Visit::Stop) => Break(Interrupt::Unwind),
        (_, Visit::Skip) => Continue(Resume::Parent),
        (_, Visit::Continue) => Continue(Resume::Siblings),
    }
}

/// Walks `items`, stopping the loop early on [`Resume::Parent`] and propagating any
/// [`Interrupt`].
fn walk_all<V, T>(
    visitor: &mut V,
    items: impl IntoIterator<Item = T>,
    walk: fn(&mut V, T) -> Walk,
) -> ControlFlow<Interrupt> {
    for item in items {
        match walk(visitor, item)? {
            Resume::Siblings => {}
            Resume::Parent => break,
        }
    }
    Continue(())
}

fn walk_root_object<V: Visitor>(visitor: &mut V, object: &RootObject) -> Walk {
    walk_inner(
        visitor,
        object,
        V::enter_root_object,
        V::exit_root_object,
        |v| walk_object(v, &object.object).map_continue(|_| ()),
    )
}

fn walk_object<V: Visitor>(visitor: &mut V, s: &Object) -> Walk {
    walk_inner(visitor, s, V::enter_object, V::exit_object, |v| {
        walk_all(v, &s.properties, walk_property)
    })
}

fn walk_property<V: Visitor>(visitor: &mut V, property: &Property) -> Walk {
    walk_inner(
        visitor,
        property,
        V::enter_property,
        V::exit_property,
        |v| walk_value(v, &property.value).map_continue(|_| ()),
    )
}

fn walk_value<V: Visitor>(visitor: &mut V, value: &Value) -> Walk {
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
                    Resume::Siblings => {}
                    // the key's exit pruned the entry's remaining sibling (its value) and,
                    // transitively, the rest of the map's entries
                    Resume::Parent => return Continue(Resume::Parent),
                }
                walk_value(v, value)
            }),
            Value::Optional {
                value: Some(inner), ..
            } => walk_value(v, inner).map_continue(|_| ()),
            _ => Continue(()),
        },
    )
}

impl Ast {
    /// Walk a [`Visitor`] implementor over every object in this tree.
    pub fn walk<V: Visitor>(&self, visitor: &mut V) {
        let _ = walk_all(visitor, &self.objects, walk_root_object);
    }
}
