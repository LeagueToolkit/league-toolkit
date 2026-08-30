use ltk_meta::PropertyKind;

use crate::{
    ast::{
        builder::Builder,
        diagnostics::Diagnostic::{self, *},
        node::TypeExpr,
    },
    cst::{ChildrenExt as _, Kind},
    parse::{Span, TokenKind},
    Node, RitoType, RitobinName as _,
};

impl<'a> Builder<'a> {
    pub fn resolve_type_expr(&mut self, tree: &Node) -> Option<TypeExpr> {
        match self.resolve_type_expr_fallable(tree) {
            Ok(rito) => Some(TypeExpr::Resolved(rito)),
            Err(e @ MissingToken(_)) => {
                self.push(e.default_span(tree.span));
                None
            }
            Err(e) => {
                self.push(e.default_span(tree.span));
                Some(TypeExpr::Unresolved)
            }
        }
    }
    pub fn resolve_type_expr_fallable(&mut self, tree: &Node) -> Result<RitoType, Diagnostic> {
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
                    .enumerate()
                    .map(|(i, t)| {
                        let resolved = PropertyKind::from_rito_name(&self.text[t.span]);
                        match resolved {
                            None => self.push(UnknownType(t.span).unwrap()),
                            Some(kind) if kind.is_container() => {
                                self.push(
                                    InvalidNesting {
                                        span: t.span,
                                        kind: RitoType::simple(kind),
                                    }
                                    .unwrap(),
                                );
                            }
                            Some(kind)
                                if base == PropertyKind::Map && i == 0 && !kind.is_primitive() =>
                            {
                                self.push(
                                    InvalidMapKey {
                                        span: t.span,
                                        kind: RitoType::simple(kind),
                                    }
                                    .unwrap(),
                                );
                            }
                            Some(_) => {}
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
