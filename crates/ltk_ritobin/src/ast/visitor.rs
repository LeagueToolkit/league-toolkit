use crate::ast::{Ast, AstObject, AstProperty, AstStruct, AstValue};

pub use crate::cst::visitor::Visit;

#[allow(unused_variables)]
pub trait Visitor {
    fn enter_object(&mut self, object: &AstObject) -> Visit {
        Visit::Continue
    }
    fn exit_object(&mut self, object: &AstObject) -> Visit {
        Visit::Continue
    }

    fn enter_struct(&mut self, s: &AstStruct) -> Visit {
        Visit::Continue
    }
    fn exit_struct(&mut self, s: &AstStruct) -> Visit {
        Visit::Continue
    }

    fn enter_property(&mut self, property: &AstProperty) -> Visit {
        Visit::Continue
    }
    fn exit_property(&mut self, property: &AstProperty) -> Visit {
        Visit::Continue
    }

    fn enter_value(&mut self, value: &AstValue) -> Visit {
        Visit::Continue
    }
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

fn enter_exit<V: Visitor, T>(
    visitor: &mut V,
    node: &T,
    enter: fn(&mut V, &T) -> Visit,
    exit: fn(&mut V, &T) -> Visit,
    children: impl FnOnce(&mut V) -> Visit,
) -> Visit {
    let ret = match enter(visitor, node) {
        Visit::Stop => Visit::Stop,
        Visit::Skip => Visit::Continue,
        Visit::Continue => match children(visitor) {
            Visit::Stop => return Visit::Stop,
            _ => Visit::Continue,
        },
    };
    if exit(visitor, node) == Visit::Stop {
        return Visit::Stop;
    }
    ret
}

/// Walks `items`, stopping early on [`Visit::Stop`] and stopping (but not failing) on
/// [`Visit::Skip`].
fn walk_all<V, T>(
    visitor: &mut V,
    items: impl IntoIterator<Item = T>,
    walk: fn(&mut V, T) -> Visit,
) -> Visit {
    for item in items {
        match walk(visitor, item) {
            Visit::Continue => {}
            Visit::Skip => break,
            Visit::Stop => return Visit::Stop,
        }
    }
    Visit::Continue
}

fn walk_object<V: Visitor>(visitor: &mut V, object: &AstObject) -> Visit {
    enter_exit(visitor, object, V::enter_object, V::exit_object, |v| {
        walk_struct(v, &object.object)
    })
}

fn walk_struct<V: Visitor>(visitor: &mut V, s: &AstStruct) -> Visit {
    enter_exit(visitor, s, V::enter_struct, V::exit_struct, |v| {
        walk_all(v, &s.properties, walk_property)
    })
}

fn walk_property<V: Visitor>(visitor: &mut V, property: &AstProperty) -> Visit {
    enter_exit(
        visitor,
        property,
        V::enter_property,
        V::exit_property,
        |v| walk_value(v, &property.value),
    )
}

fn walk_value<V: Visitor>(visitor: &mut V, value: &AstValue) -> Visit {
    enter_exit(
        visitor,
        value,
        V::enter_value,
        V::exit_value,
        |v| match value {
            AstValue::Struct(s) | AstValue::Embedded(s) => walk_struct(v, s),
            AstValue::Container { items, .. } | AstValue::UnorderedContainer { items, .. } => {
                walk_all(v, items, walk_value)
            }
            AstValue::Map { entries, .. } => walk_all(v, entries, |v, (key, value)| {
                match walk_value(v, key) {
                    Visit::Stop => return Visit::Stop,
                    Visit::Skip | Visit::Continue => {}
                }
                walk_value(v, value)
            }),
            AstValue::Optional {
                value: Some(inner), ..
            } => walk_value(v, inner),
            _ => Visit::Continue,
        },
    )
}

impl Ast {
    /// Walk a [`Visitor`] implementor over every object in this tree.
    pub fn walk<V: Visitor>(&self, visitor: &mut V) {
        let _ = walk_all(visitor, &self.objects, walk_object);
    }
}
