use std::ops::ControlFlow::{self, Break, Continue};

use crate::ast::{Ast, AstObject, AstProperty, AstStruct, AstValue};

pub use crate::cst::visitor::Visit;

#[allow(unused_variables)]
/// [Visitor pattern](https://rust-unofficial.github.io/patterns/patterns/behavioural/visitor.html)
/// for walking an [`Ast`].
///
/// Every node kind has a matching `enter_*`/`exit_*` pair, and both run for every node that's
/// entered - even one whose `enter_*` returned [`Visit::Skip`], and even while the walk is
/// unwinding after a [`Visit::Stop`] elsewhere in the tree - unless a [`Visit::Abort`] cuts the
/// walk short first, in which case no further callbacks of either kind run.
///
/// [`Visit::Skip`] means something different depending on which callback returns it:
/// - from `enter_*`: this node's children are not visited, but its `exit_*` still runs.
/// - from `exit_*`: the *parent's* remaining children are pruned - the walk jumps straight to the
///   parent's `exit_*` and continues from there. What counts as a "child" is spelled out per pair
///   below.
pub trait Visitor {
    /// Called on first discovery of an [`AstObject`], before its struct.
    fn enter_object(&mut self, object: &AstObject) -> Visit {
        Visit::Continue
    }
    /// Called after `object`'s struct has been walked, or would have been but was skipped.
    ///
    /// A [`Visit::Skip`] here prunes the remaining sibling objects: the walk ends without
    /// visiting any objects after this one.
    fn exit_object(&mut self, object: &AstObject) -> Visit {
        Visit::Continue
    }

    /// Called on first discovery of an [`AstStruct`], before its properties.
    fn enter_struct(&mut self, s: &AstStruct) -> Visit {
        Visit::Continue
    }
    /// Called after a struct's properties have been walked, or would have been but were skipped.
    ///
    /// A [`Visit::Skip`] here prunes this struct's remaining sibling properties, if any - it has
    /// no effect on an [`AstObject`]'s struct, which is its only child.
    fn exit_struct(&mut self, s: &AstStruct) -> Visit {
        Visit::Continue
    }

    /// Called on first discovery of an [`AstProperty`], before walking its value.
    fn enter_property(&mut self, property: &AstProperty) -> Visit {
        Visit::Continue
    }
    /// Called after a property's value has been walked, or would have been but was skipped.
    ///
    /// A [`Visit::Skip`] here prunes this property's remaining sibling properties within its
    /// enclosing struct.
    fn exit_property(&mut self, property: &AstProperty) -> Visit {
        Visit::Continue
    }

    /// Called on first discovery of an [`AstValue`], before whatever it contains: a nested
    /// struct, a container's items, a map's key/value entries, or an optional's inner value.
    /// Values with no such contents (scalars, an empty optional) have no children to skip.
    fn enter_value(&mut self, value: &AstValue) -> Visit {
        Visit::Continue
    }
    /// Called after a value has been walked, or would have been but were skipped.
    ///
    /// A [`Visit::Skip`] here prunes whatever remains at this value's level: the rest of a
    /// container's items, the rest of a map's entries (a map entry's value is itself skippable
    /// this way - it prunes the remaining entries), or for a property's value or an object's
    /// struct, which have no siblings - nothing observable at all.
    fn exit_value(&mut self, value: &AstValue) -> Visit {
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

fn walk_object<V: Visitor>(visitor: &mut V, object: &AstObject) -> Walk {
    walk_inner(visitor, object, V::enter_object, V::exit_object, |v| {
        walk_struct(v, &object.object).map_continue(|_| ())
    })
}

fn walk_struct<V: Visitor>(visitor: &mut V, s: &AstStruct) -> Walk {
    walk_inner(visitor, s, V::enter_struct, V::exit_struct, |v| {
        walk_all(v, &s.properties, walk_property)
    })
}

fn walk_property<V: Visitor>(visitor: &mut V, property: &AstProperty) -> Walk {
    walk_inner(
        visitor,
        property,
        V::enter_property,
        V::exit_property,
        |v| walk_value(v, &property.value).map_continue(|_| ()),
    )
}

fn walk_value<V: Visitor>(visitor: &mut V, value: &AstValue) -> Walk {
    walk_inner(
        visitor,
        value,
        V::enter_value,
        V::exit_value,
        |v| match value {
            AstValue::Struct(s) | AstValue::Embedded(s) => walk_struct(v, s).map_continue(|_| ()),
            AstValue::Container { items, .. } | AstValue::UnorderedContainer { items, .. } => {
                walk_all(v, items, walk_value)
            }
            AstValue::Map { entries, .. } => walk_all(v, entries, |v, (key, value)| {
                match walk_value(v, key)? {
                    Resume::Siblings => {}
                    // the key's exit pruned the entry's remaining sibling (its value) and,
                    // transitively, the rest of the map's entries
                    Resume::Parent => return Continue(Resume::Parent),
                }
                walk_value(v, value)
            }),
            AstValue::Optional {
                value: Some(inner), ..
            } => walk_value(v, inner).map_continue(|_| ()),
            _ => Continue(()),
        },
    )
}

impl Ast {
    /// Walk a [`Visitor`] implementor over every object in this tree.
    pub fn walk<V: Visitor>(&self, visitor: &mut V) {
        let _ = walk_all(visitor, &self.objects, walk_object);
    }
}
