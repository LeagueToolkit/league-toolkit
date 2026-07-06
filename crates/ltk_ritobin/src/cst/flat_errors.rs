use crate::{
    cst::{
        visitor::{Visit, VisitCtx, Visitor},
        Cst, NodeId,
    },
    parse::Error,
};

#[derive(Default)]
pub struct FlatErrors {
    errors: Vec<Error>,
}

impl FlatErrors {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn into_errors(self) -> Vec<Error> {
        self.errors
    }

    pub fn walk(tree: &Cst) -> Vec<Error> {
        let mut errors = Self::new();
        tree.walk(&mut errors);
        errors.into_errors()
    }
}

impl Visitor for FlatErrors {
    fn exit_tree(&mut self, ctx: &VisitCtx, tree: NodeId) -> Visit {
        let tree = ctx.node(tree).unwrap();
        self.errors.extend_from_slice(&tree.errors);
        Visit::Continue
    }
}
