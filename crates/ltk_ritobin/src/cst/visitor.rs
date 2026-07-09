//! Visitor pattern for walking CSTs
use super::{tree::Child, Cst};
use crate::cst::{Node, NodeId, TokenId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Visit {
    /// Stop walking immediately
    Stop,
    /// Skips all remaining tokens in the current tree
    Skip,
    /// Continue walking
    Continue,
}

pub struct VisitCtx<'arena> {
    pub cst: &'arena Cst<'arena>,
}
impl<'arena> VisitCtx<'arena> {
    pub fn node(&self, id: NodeId) -> Option<&Node<'arena>> {
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

    /// Called after all children of a [`Node`] have finished walking.
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

impl Cst<'_> {
    /// Walk a [`Visitor`] implementor along this tree.
    pub fn walk<V: Visitor>(&self, visitor: &mut V) {
        if self.nodes.is_empty() {
            return;
        }
        self.walk_inner(visitor, NodeId(0));
    }

    fn walk_inner<V: Visitor>(&self, visitor: &mut V, node_idx: NodeId) -> Visit {
        let ctx = VisitCtx { cst: self };

        let node = self.node(node_idx).unwrap();
        if let Some(ret) = match visitor.enter_tree(&ctx, node_idx) {
            Visit::Stop => Some(Visit::Stop),
            Visit::Skip => Some(Visit::Continue),
            _ => None,
        } {
            if visitor.exit_tree(&ctx, node_idx) == Visit::Stop {
                return Visit::Stop;
            }
            return ret;
        }

        for child in node.children.get(self) {
            match child {
                Child::Token(token) => match visitor.visit_token(&ctx, *token, node_idx) {
                    Visit::Continue => {}
                    Visit::Skip => break,
                    Visit::Stop => return Visit::Stop,
                },
                Child::Tree(child) => match self.walk_inner(visitor, *child) {
                    Visit::Continue => {}
                    Visit::Skip => {
                        break;
                    }
                    Visit::Stop => return Visit::Stop,
                },
            }
        }

        visitor.exit_tree(&ctx, node_idx)
    }
}

// #[cfg(test)]
// mod test {
//     use crate::{
//         cst::{Node, Visitor},
//         parse::Span,
//         Cst,
//     };
//     use bumpalo::{vec, Bump};
//
//     #[derive(Default)]
//     struct TestVisitor {
//         seen: Vec<()>,
//     }
//
//     impl Visitor for TestVisitor {
//         fn enter_tree(
//             &mut self,
//             ctx: &super::VisitCtx<'_>,
//             tree: crate::cst::NodeId,
//         ) -> super::Visit {
//             println!("tree: {tree:?}");
//             super::Visit::Continue
//         }
//
//         fn exit_tree(
//             &mut self,
//             ctx: &super::VisitCtx<'_>,
//             tree: crate::cst::NodeId,
//         ) -> super::Visit {
//             super::Visit::Continue
//         }
//
//         fn visit_token(
//             &mut self,
//             ctx: &super::VisitCtx<'_>,
//             token: crate::parse::Token,
//             parent: crate::cst::NodeId,
//         ) -> super::Visit {
//             super::Visit::Continue
//         }
//     }
//
//     #[test]
//     fn visitor() {
//         let bump = Bump::new();
//         let cst = Cst {
//             nodes: vec![in &bump; Node {span: Span::default(), kind: crate::cst::Kind::Class, children: vec![in &bump;], errors: vec![in &bump;]}],
//         };
//
//         let mut visitor = TestVisitor::default();
//         cst.walk(&mut visitor);
//         assert!(false);
//     }
// }
