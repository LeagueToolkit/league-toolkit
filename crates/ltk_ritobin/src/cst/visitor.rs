//! Visitor pattern for walking CSTs
use std::ops::ControlFlow::{self, Break, Continue};

use super::{tree::Child, Cst};
use crate::cst::{Node, NodeId, TokenId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Visit {
    /// Stop the walk
    ///
    /// The walk will unwind, calling [`Visitor::exit_tree`] for every node that was in the walk stack,
    /// bottom-up, until the walk is fully unwound. The walk will not resume after a [`Visit::Stop`].
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
    /// even if the walk is unwinding after a [`Visit::Stop`].
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
}

/// Walk unwind marker, used to propagate a [`Visit::Stop`] up the walk stack.
struct Unwind;

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
/// - `Break(Unwind)` propagates a [`Visit::Stop`] and `?` on it unwinds the walk.
type Walk = ControlFlow<Unwind, Resume>;

impl Cst {
    /// Walk a [`Visitor`] implementor along this tree.
    pub fn walk<V: Visitor>(&self, visitor: &mut V) -> WalkOutcome {
        if self.nodes.is_empty() {
            return WalkOutcome::Completed;
        }

        match self.walk_inner(visitor, NodeId(0)) {
            Continue(_) => WalkOutcome::Completed,
            Break(Unwind) => WalkOutcome::Stopped,
        }
    }

    fn walk_inner<V: Visitor>(&self, visitor: &mut V, node_idx: NodeId) -> Walk {
        let ctx = VisitCtx { cst: self };

        let walked = match visitor.enter_tree(&ctx, node_idx) {
            Visit::Stop => Break(Unwind),
            Visit::Skip => Continue(()),
            Visit::Continue => self.walk_children(visitor, &ctx, node_idx),
        };

        // exit_tree runs exactly once for every entered node, even while unwinding
        match (walked, visitor.exit_tree(&ctx, node_idx)) {
            (Break(Unwind), _) | (_, Visit::Stop) => Break(Unwind),
            (_, Visit::Skip) => Continue(Resume::Parent),
            (_, Visit::Continue) => Continue(Resume::Siblings),
        }
    }

    fn walk_children<V: Visitor>(
        &self,
        visitor: &mut V,
        ctx: &VisitCtx<'_>,
        node_idx: NodeId,
    ) -> ControlFlow<Unwind> {
        for child in self.node(node_idx).unwrap().children.get(self) {
            match child {
                Child::Token(token) => match visitor.visit_token(ctx, *token, node_idx) {
                    Visit::Continue => {}
                    Visit::Skip => break,
                    Visit::Stop => return Break(Unwind),
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::cst::Kind;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Event {
        Enter(Kind),
        Token,
        Exit(Kind),
    }

    /// Records every callback; optionally returns Stop/Skip when a node of the
    /// configured kind is hit.
    #[derive(Default)]
    struct Recorder {
        events: Vec<Event>,
        stop_on_enter: Option<Kind>,
        skip_on_enter: Option<Kind>,
        stop_on_exit: Option<Kind>,
        skip_on_exit: Option<Kind>,
        stop_on_token: bool,
        skip_on_token_in: Option<Kind>,
    }

    impl Recorder {
        fn walk(mut self, text: &str) -> Vec<Event> {
            let cst = Cst::parse(text);
            assert!(cst.errors.is_empty(), "parse errors: {:#?}", cst.errors);
            cst.walk(&mut self);
            self.events
        }
    }

    impl Visitor for Recorder {
        fn enter_tree(&mut self, ctx: &VisitCtx<'_>, tree: NodeId) -> Visit {
            let kind = ctx.node(tree).unwrap().kind;
            self.events.push(Event::Enter(kind));
            if self.stop_on_enter == Some(kind) {
                return Visit::Stop;
            }
            if self.skip_on_enter == Some(kind) {
                return Visit::Skip;
            }
            Visit::Continue
        }
        fn exit_tree(&mut self, ctx: &VisitCtx<'_>, tree: NodeId) -> Visit {
            let kind = ctx.node(tree).unwrap().kind;
            self.events.push(Event::Exit(kind));
            if self.stop_on_exit == Some(kind) {
                return Visit::Stop;
            }
            if self.skip_on_exit == Some(kind) {
                return Visit::Skip;
            }
            Visit::Continue
        }
        fn visit_token(&mut self, ctx: &VisitCtx<'_>, _token: TokenId, parent: NodeId) -> Visit {
            self.events.push(Event::Token);
            if self.stop_on_token {
                return Visit::Stop;
            }
            if self.skip_on_token_in == Some(ctx.node(parent).unwrap().kind) {
                return Visit::Skip;
            }
            Visit::Continue
        }
    }

    const TEXT: &str = "a: list[u32] = { 1 2 }\nb: u32 = 3";

    /// Every `Enter` has a matching, properly nested `Exit`, and nothing is
    /// left open at the end.
    fn assert_balanced(events: &[Event]) {
        let mut stack = Vec::new();
        for event in events {
            match event {
                Event::Enter(kind) => stack.push(*kind),
                Event::Exit(kind) => {
                    assert_eq!(stack.pop(), Some(*kind), "mismatched exit in {events:#?}")
                }
                Event::Token => {}
            }
        }
        assert!(stack.is_empty(), "nodes never exited: {stack:?}");
    }

    fn count(events: &[Event], event: Event) -> usize {
        events.iter().filter(|e| **e == event).count()
    }

    #[test]
    fn full_walk_is_balanced() {
        let events = Recorder::default().walk(TEXT);
        assert_balanced(&events);
        assert_eq!(events.first(), Some(&Event::Enter(Kind::File)));
        assert_eq!(events.last(), Some(&Event::Exit(Kind::File)));
        assert_eq!(count(&events, Event::Enter(Kind::Entry)), 2);
    }

    #[test]
    fn stop_from_enter_exits_open_ancestors() {
        let events = Recorder {
            stop_on_enter: Some(Kind::EntryValue),
            ..Default::default()
        }
        .walk(TEXT);
        assert_balanced(&events);
        // nothing new is entered after the stop fired
        let stop_at = events
            .iter()
            .position(|e| *e == Event::Enter(Kind::EntryValue))
            .unwrap();
        assert!(
            events[stop_at + 1..]
                .iter()
                .all(|e| matches!(e, Event::Exit(_))),
            "walk continued after Stop: {events:#?}"
        );
    }

    #[test]
    fn stop_from_token_exits_open_ancestors() {
        let events = Recorder {
            stop_on_token: true,
            ..Default::default()
        }
        .walk(TEXT);
        assert_balanced(&events);
        assert_eq!(count(&events, Event::Token), 1);
    }

    #[test]
    fn stop_from_exit_exits_open_ancestors() {
        let events = Recorder {
            stop_on_exit: Some(Kind::EntryKey),
            ..Default::default()
        }
        .walk(TEXT);
        assert_balanced(&events);
        let stop_at = events
            .iter()
            .position(|e| *e == Event::Exit(Kind::EntryKey))
            .unwrap();
        assert!(
            events[stop_at + 1..]
                .iter()
                .all(|e| matches!(e, Event::Exit(_))),
            "walk continued after Stop: {events:#?}"
        );
    }

    #[test]
    fn skip_from_enter_skips_children_but_still_exits() {
        let events = Recorder {
            skip_on_enter: Some(Kind::Entry),
            ..Default::default()
        }
        .walk(TEXT);
        assert_balanced(&events);
        // children of both entries were skipped, the entries still exited,
        // and the walk went on to the sibling entry
        assert_eq!(count(&events, Event::Enter(Kind::EntryKey)), 0);
        assert_eq!(count(&events, Event::Enter(Kind::Entry)), 2);
        assert_eq!(count(&events, Event::Exit(Kind::Entry)), 2);
    }

    #[test]
    fn skip_from_token_skips_the_nodes_remaining_children() {
        // skip fires on the block's `{`, so its list items are never entered
        let events = Recorder {
            skip_on_token_in: Some(Kind::Block),
            ..Default::default()
        }
        .walk(TEXT);
        assert_balanced(&events);
        assert_eq!(count(&events, Event::Enter(Kind::ListItem)), 0);
        assert_eq!(count(&events, Event::Exit(Kind::Block)), 1);
    }

    #[test]
    fn skip_from_exit_prunes_later_siblings() {
        let events = Recorder {
            skip_on_exit: Some(Kind::Entry),
            ..Default::default()
        }
        .walk(TEXT);
        assert_balanced(&events);
        // the first entry's exit returned Skip, so the second entry is pruned,
        // but the parent still exits normally
        assert_eq!(count(&events, Event::Enter(Kind::Entry)), 1);
        assert_eq!(events.last(), Some(&Event::Exit(Kind::File)));
    }

    #[test]
    fn pruning_is_local_and_the_walk_continues_elsewhere() {
        // the first list item's exit prunes the block's remaining children,
        // but everything outside the block is still walked
        let events = Recorder {
            skip_on_exit: Some(Kind::ListItem),
            ..Default::default()
        }
        .walk(TEXT);
        assert_balanced(&events);
        assert_eq!(count(&events, Event::Enter(Kind::ListItem)), 1);
        assert_eq!(count(&events, Event::Enter(Kind::Entry)), 2);
    }

    #[test]
    fn walk_reports_how_it_ended() {
        let cst = Cst::parse(TEXT);
        assert_eq!(cst.walk(&mut Recorder::default()), WalkOutcome::Completed);
        assert_eq!(
            cst.walk(&mut Recorder {
                stop_on_token: true,
                ..Default::default()
            }),
            WalkOutcome::Stopped
        );
        // pruning is not a stop
        assert_eq!(
            cst.walk(&mut Recorder {
                skip_on_exit: Some(Kind::Entry),
                ..Default::default()
            }),
            WalkOutcome::Completed
        );
    }
}
