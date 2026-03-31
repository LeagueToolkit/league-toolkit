//! Visitor pattern for walking CSTs
use super::{tree::Child, Cst};
use crate::parse::tokenizer::Token;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Visit {
    /// Stop walking immediately
    Stop,
    /// Skips all remaining tokens in the current tree
    Skip,
    /// Continue walking
    Continue,
}

#[allow(unused_variables)]
/// [Visitor pattern](https://rust-unofficial.github.io/patterns/patterns/behavioural/visitor.html) for easily walking [`Cst`]s
pub trait Visitor {
    /// Called on first discovery of a [`Cst`], before any children of that node.
    #[must_use]
    fn enter_tree(&mut self, tree: &Cst) -> Visit {
        Visit::Continue
    }

    /// Called after all children of a [`Cst`] have finished walking.
    #[must_use]
    fn exit_tree(&mut self, tree: &Cst) -> Visit {
        Visit::Continue
    }

    /// Called on every token walked, with the node the token was found in provided as context.
    #[must_use]
    fn visit_token(&mut self, token: &Token, context: &Cst) -> Visit {
        Visit::Continue
    }
}

pub trait VisitorExt: Sized + Visitor {
    fn walk(mut self, tree: &Cst) -> Self {
        tree.walk(&mut self);
        self
    }
}

impl<T: Sized + Visitor> VisitorExt for T {}

impl Cst {
    /// Walk a [`Visitor`] implementor along this tree.
    pub fn walk<V: Visitor>(&self, visitor: &mut V) {
        self.walk_inner(visitor);
    }

    fn walk_inner<V: Visitor>(&self, visitor: &mut V) -> Visit {
        if let Some(ret) = match visitor.enter_tree(self) {
            Visit::Stop => Some(Visit::Stop),
            Visit::Skip => Some(Visit::Continue),
            _ => None,
        } {
            if visitor.exit_tree(self) == Visit::Stop {
                return Visit::Stop;
            }
            return ret;
        }

        for child in &self.children {
            match child {
                Child::Token(token) => match visitor.visit_token(token, self) {
                    Visit::Continue => {}
                    Visit::Skip => break,
                    Visit::Stop => return Visit::Stop,
                },
                Child::Tree(child_tree) => match child_tree.walk_inner(visitor) {
                    Visit::Continue => {}
                    Visit::Skip => {
                        break;
                    }
                    Visit::Stop => return Visit::Stop,
                },
            }
        }

        visitor.exit_tree(self)
    }
}
