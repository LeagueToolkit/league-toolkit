use ltk_meta::PropertyKind;

use crate::{
    ast::build::{BuildCtx, ChildrenExt as _},
    cst::Kind,
    parse::{Span, TokenKind},
    typecheck::diagnostics::Diagnostic::{self, *},
    Node, RitoType, RitobinName as _,
};

impl<'a> BuildCtx<'a> {
    pub fn resolve_type_expr(&mut self, tree: &Node) -> Result<RitoType, Diagnostic> {
        let children = tree.children.get(self.cst);

        let base = children
            .find_token(self.cst, TokenKind::Name)
            .ok_or(MissingToken(TokenKind::Name))?;
        let base_span = base.span;
        let base =
            PropertyKind::from_rito_name(&self.text[base.span]).ok_or(UnknownType(base.span))?;

        let subtypes = match children.find_tree(self.cst, Kind::TypeArgList) {
            Some(subtypes_node) => {
                let subtypes_span = subtypes_node.span;
                let expected = base.subtype_count();

                if expected == 0 {
                    return Err(UnexpectedSubtypes {
                        span: subtypes_span,
                        base_type: base_span,
                    });
                }

                let subtypes = subtypes_node
                    .children
                    .get(self.cst)
                    .iter()
                    .filter_map(|c| c.tree(self.cst).filter(|t| t.kind == Kind::TypeArg))
                    .map(|t| {
                        let resolved = PropertyKind::from_rito_name(&self.text[t.span]);
                        if resolved.is_none() {
                            self.push(UnknownType(t.span).unwrap());
                        }
                        (resolved, t.span)
                    })
                    .collect::<Vec<_>>();

                if subtypes.len() != expected.into() {
                    let span = if subtypes.len() > expected.into() {
                        subtypes[expected as _..]
                            .iter()
                            .map(|s| s.1)
                            .reduce(|acc, s| Span::new(acc.start, s.end))
                            .unwrap_or(subtypes_span)
                    } else {
                        subtypes.last().map(|s| s.1).unwrap_or(subtypes_span)
                    };
                    return Err(SubtypeCountMismatch {
                        span,
                        got: subtypes.len() as u8,
                        expected,
                    });
                }

                let mut subtypes = subtypes.iter();
                [
                    subtypes.next().and_then(|s| s.0),
                    subtypes.next().and_then(|s| s.0),
                ]
            }
            None => [None, None],
        };

        Ok(RitoType { base, subtypes })
    }
}
