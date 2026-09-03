use super::iter::*;
use crate::ast::{
    node::{NodeRef, SubNodeRef},
    Ast,
};

impl Ast {
    /// The chain of nodes on the way to `offset`, outermost first.
    pub fn coarse_path_to(&self, offset: u32) -> AstPathIter<'_> {
        AstPathIter::from_ast(self, offset)
    }
    /// The chain of nodes on the way to `offset`, outermost first
    pub fn fine_path_to(&self, offset: u32) -> AstFinePathIter<'_> {
        AstFinePathIter::from_ast(self, offset)
    }

    /// The most specific node containing `offset`. See [`Self::path_to`] if you need the full path.
    pub fn coarse_find_node(&self, offset: u32) -> Option<NodeRef<'_>> {
        self.coarse_path_to(offset).last()
    }
    pub fn fine_find_node(&self, offset: u32) -> Option<SubNodeRef<'_>> {
        self.fine_path_to(offset).last()
    }
}

// impl<'a> NodeRef<'a> {
//     /// The chain of nodes on the way to `offset`, including this node.
//     pub fn path_to(&self, offset: u32) -> AstPathIter<'a> {
//         AstPathIter::from_node(*self, offset)
//     }
// }
