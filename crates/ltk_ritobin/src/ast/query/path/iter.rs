use crate::ast::{
    node::{NodeExt as _, NodeRef, SubNodeRef},
    Ast,
};

/// Iterator of every [`Node`] on the way to a given offset, from the top level.
///
/// Use [`Ast::coarse_path_to`] to construct this iterator.
#[derive(Clone)]
pub struct AstPathIter<'a> {
    next: Option<NodeRef<'a>>,
    offset: u32,
}

impl<'a> AstPathIter<'a> {
    pub(crate) fn from_ast(ast: &'a Ast, offset: u32) -> Self {
        Self {
            next: ast
                .objects
                .iter()
                .find(|o| o.span().contains(offset))
                .map(NodeRef::Object),
            offset,
        }
    }
    pub(crate) fn from_node(node: NodeRef<'a>, offset: u32) -> Self {
        Self {
            next: node.span().contains(offset).then_some(node),
            offset,
        }
    }
}

impl<'a> Iterator for AstPathIter<'a> {
    type Item = NodeRef<'a>;

    fn next(&mut self) -> Option<NodeRef<'a>> {
        let current = self.next.take()?;
        self.next = current.children().find(|c| c.span().contains(self.offset));
        Some(current)
    }
}

/// Iterator of every [`Node`] on the way to a given offset, from the top level.
///
/// Use [`Ast::fine_path_to`] to construct this iterator.
#[derive(Clone)]
pub struct AstFinePathIter<'a> {
    next: Option<SubNodeRef<'a>>,
    offset: u32,
}
impl<'a> AstFinePathIter<'a> {
    pub(crate) fn from_ast(ast: &'a Ast, offset: u32) -> Self {
        Self {
            next: ast
                .objects
                .iter()
                .find(|o| o.span().contains(offset))
                .map(SubNodeRef::from),
            offset,
        }
    }
    // pub(crate) fn from_node(node: Node<'a>, offset: u32) -> Self {
    //     Self {
    //         next: node.span().contains(offset).then_some(node),
    //         offset,
    //     }
    // }
}

impl<'a> Iterator for AstFinePathIter<'a> {
    type Item = SubNodeRef<'a>;

    fn next(&mut self) -> Option<SubNodeRef<'a>> {
        let current = self.next.take()?;

        // only recurse on nodes with detail = Node, since that means we have more resolution
        if current.detail().is_node() {
            self.next = match current {
                SubNodeRef::Object(v, _) => v
                    .detailed_children()
                    .find(|c| c.span().is_some_and(|span| span.contains(self.offset))),
                SubNodeRef::Struct(v, _) => v
                    .detailed_children()
                    .find(|c| c.span().is_some_and(|span| span.contains(self.offset))),
                SubNodeRef::Property(v, _) => v
                    .detailed_children()
                    .find(|c| c.span().is_some_and(|span| span.contains(self.offset))),
                SubNodeRef::Value(v) => v
                    .children()
                    .find(|c| c.span().contains(self.offset))
                    .map(|c| c.into()),
            };
        }

        Some(current)
    }
}
