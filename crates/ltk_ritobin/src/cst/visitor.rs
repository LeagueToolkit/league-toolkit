//! Visitor pattern for walking CSTs
use std::ops::ControlFlow::{self, Break, Continue};

use super::{tree::Child, Cst};
use crate::cst::{Node, NodeId, TokenId};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Visit {
    /// Aborts the walk immediately, with no stack unwinding.
    ///
    /// [`Visitor::exit_tree`] will not be called for open nodes in the walk stack.
    Abort,
    /// Stop the walk
    ///
    /// The walk will unwind, calling [`Visitor::exit_tree`] for every node that was in the walk stack,
    /// bottom-up, until the walk is fully unwound. The walk will not resume after a [`Visit::Stop`].
    /// Use [`Visit::Abort`] to bail without the exit calls.
    Stop,
    /// Skip ahead, locally
    ///
    /// - From [`Visitor::enter_tree`]: the node's children are skipped; its
    ///   [`Visitor::exit_tree`] still runs.
    /// - From [`Visitor::visit_token`]: the rest of the current node's children are skipped.
    /// - From [`Visitor::exit_tree`]: the parent's remaining children are pruned - the walk
    ///   jumps straight to the parent's [`Visitor::exit_tree`] and continues from there.
    Skip,
    /// Continue walking
    Continue,
}

pub struct VisitCtx<'a> {
    pub cst: &'a Cst,
}
impl<'a> VisitCtx<'a> {
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.cst.node(id)
    }
}

#[allow(unused_variables)]
/// [Visitor pattern](https://rust-unofficial.github.io/patterns/patterns/behavioural/visitor.html) for easily walking [`Node`]s
pub trait Visitor {
    /// Called on first discovery of a [`Node`], before any children of that node.
    #[must_use]
    fn enter_tree(&mut self, ctx: &VisitCtx<'_>, tree: NodeId) -> Visit {
        Visit::Continue
    }

    /// Called when a [`Node`] finished its walk, got skipped, or the walk stack is unwinding.
    ///
    /// Runs symmetrically to [`Visitor::enter_tree`], so every node that was entered will be exited,
    /// even if the walk is unwinding after a [`Visit::Stop`] - unless the walk is aborted by a
    /// [`Visit::Abort`], which skips all remaining callbacks.
    ///
    /// Returning [`Visit::Skip`] from here prunes the parent's remaining children: the walk
    /// jumps straight to the parent's `exit_tree` and continues from there.
    #[must_use]
    fn exit_tree(&mut self, ctx: &VisitCtx<'_>, tree: NodeId) -> Visit {
        Visit::Continue
    }

    /// Called on every token walked, with the node the token was found in provided as context.
    #[must_use]
    fn visit_token(&mut self, ctx: &VisitCtx<'_>, token: TokenId, parent: NodeId) -> Visit {
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

/// How a [`Cst::walk`] ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkOutcome {
    /// The walk reached the end of the tree.
    Completed,
    /// A visitor returned [`Visit::Stop`] and the walk unwound early.
    Stopped,
    /// A visitor returned [`Visit::Abort`] and the walk ended without unwinding.
    Aborted,
}

/// Walk teardown marker, propagated up the walk stack.
enum Interrupt {
    /// A [`Visit::Stop`]: [`Visitor::exit_tree`] still runs for every open node, bottom-up.
    Unwind,
    /// A [`Visit::Abort`]: no further callbacks run.
    Abort,
}

/// Where the walk resumes after a child subtree finished.
enum Resume {
    /// With the parent's remaining children.
    Siblings,
    /// At the parent's [`Visitor::exit_tree`]: the child's exit returned
    /// [`Visit::Skip`], pruning the remaining siblings.
    Parent,
}

/// Subtree walk state
///
/// - `Continue(resume)` continues the walk; `?` on it yields where the walk
///   resumes - with the next sibling, or at the parent.
/// - `Break(interrupt)` tears the walk down; `?` on it propagates the teardown.
type Walk = ControlFlow<Interrupt, Resume>;

impl Cst {
    /// Walk a [`Visitor`] implementor along this tree.
    pub fn walk<V: Visitor>(&self, visitor: &mut V) -> WalkOutcome {
        if self.nodes.is_empty() {
            return WalkOutcome::Completed;
        }

        match self.walk_inner(visitor, NodeId(0)) {
            Continue(_) => WalkOutcome::Completed,
            Break(Interrupt::Unwind) => WalkOutcome::Stopped,
            Break(Interrupt::Abort) => WalkOutcome::Aborted,
        }
    }

    fn walk_inner<V: Visitor>(&self, visitor: &mut V, node_idx: NodeId) -> Walk {
        let ctx = VisitCtx { cst: self };

        let walked = match visitor.enter_tree(&ctx, node_idx) {
            Visit::Abort => Break(Interrupt::Abort),
            Visit::Stop => Break(Interrupt::Unwind),
            Visit::Skip => Continue(()),
            Visit::Continue => self.walk_children(visitor, &ctx, node_idx),
        };

        // an abort skips the remaining exits entirely
        if let Break(Interrupt::Abort) = walked {
            return Break(Interrupt::Abort);
        }

        // exit_tree runs exactly once for every entered node, even while unwinding
        match (walked, visitor.exit_tree(&ctx, node_idx)) {
            (_, Visit::Abort) => Break(Interrupt::Abort),
            (Break(Interrupt::Unwind), _) | (_, Visit::Stop) => Break(Interrupt::Unwind),
            (_, Visit::Skip) => Continue(Resume::Parent),
            (_, Visit::Continue) => Continue(Resume::Siblings),
        }
    }

    fn walk_children<V: Visitor>(
        &self,
        visitor: &mut V,
        ctx: &VisitCtx<'_>,
        node_idx: NodeId,
    ) -> ControlFlow<Interrupt> {
        for child in self.node(node_idx).unwrap().children.get(self) {
            match child {
                Child::Token(token) => match visitor.visit_token(ctx, *token, node_idx) {
                    Visit::Continue => {}
                    Visit::Skip => break,
                    Visit::Stop => return Break(Interrupt::Unwind),
                    Visit::Abort => return Break(Interrupt::Abort),
                },
                Child::Tree(child) => match self.walk_inner(visitor, *child)? {
                    Resume::Siblings => {}
                    Resume::Parent => break,
                },
            }
        }

        Continue(())
    }
}
