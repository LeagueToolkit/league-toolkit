use ltk_hash::Hasher;
use ltk_meta::PropertyKind;

use crate::span::Span;
use crate::{
    ast::{
        builder::Builder,
        diagnostics::Diagnostic::{self, *},
        node::TypeExpr,
    },
    cst::{ChildrenExt as _, Kind},
    hash_algorithm_from_name, hash_width_from_name,
    parse::TokenKind,
    rito_hasher, Node, RitoType, RitobinName as _,
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

        let args = children.find_tree(self.cst, Kind::TypeArgList);
        if base == PropertyKind::Hash {
            return Ok(RitoType::hash(match args {
                Some(args) => self.resolve_hasher(args)?,
                None => Hasher::DEFAULT,
            }));
        }

        let mut hashers = [Hasher::DEFAULT; 2];
        let subtypes = match args {
            Some(subtypes_node) => {
                let subtypes_span = subtypes_node.span;
                let expected = base.subtype_count();

                if expected == 0 {
                    return Err(UnexpectedSubtypes {
                        span: subtypes_span,
                        base_type: base_span,
                    });
                }

                let subtypes = self
                    .type_args(subtypes_node)
                    .into_iter()
                    .enumerate()
                    .map(|(i, t)| {
                        let arg = t.children.get(self.cst);
                        let name_span = arg
                            .find_token(self.cst, TokenKind::Name)
                            .map_or(t.span, |name| name.span);
                        let resolved = PropertyKind::from_rito_name(&self.text[name_span]);
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
                                if base == PropertyKind::Map
                                    && i == 0
                                    && !kind.is_valid_map_key() =>
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
                        if let Some(nested) = arg.find_tree(self.cst, Kind::TypeArgList) {
                            match resolved {
                                Some(PropertyKind::Hash) => match self.resolve_hasher(nested) {
                                    Ok(hasher) if i < hashers.len() => hashers[i] = hasher,
                                    Ok(_) => {}
                                    Err(e) => self.push(e.default_span(nested.span)),
                                },
                                Some(_) => self.push(
                                    UnexpectedSubtypes {
                                        span: nested.span,
                                        base_type: name_span,
                                    }
                                    .unwrap(),
                                ),
                                None => {}
                            }
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

        let mut rito = RitoType::new(base, subtypes);
        rito.hashers[1..].copy_from_slice(&hashers);
        Ok(rito)
    }

    /// Returns the `TypeArg` nodes of a `TypeArgList` node.
    fn type_args(&self, list: &Node) -> Vec<&'a Node> {
        let cst = self.cst;
        list.children
            .get(cst)
            .iter()
            .filter_map(|c| c.tree(cst).filter(|t| t.kind == Kind::TypeArg))
            .collect()
    }

    /// Resolves the type arguments `[width,algorithm]` of a `hash` type to a hasher.
    ///
    /// # Errors
    /// Fails if the list does not have 2 arguments, if the first argument is not `u32` or `u64`,
    /// or if the second argument is not `fnv1a`, `xxh64` or `xxh3`.
    fn resolve_hasher(&self, list: &Node) -> Result<Hasher, Diagnostic> {
        let args = self.type_args(list);
        let [width, algorithm] = args[..] else {
            let span = match args.get(2..) {
                Some([first, .., last]) => Span::new(first.span.start, last.span.end),
                Some([only]) => only.span,
                _ => args.last().map_or(list.span, |arg| arg.span),
            };
            return Err(SubtypeCountMismatch {
                span,
                got: args.len() as u8,
                expected: 2,
            });
        };
        let width =
            hash_width_from_name(&self.text[width.span]).ok_or(UnknownHashWidth(width.span))?;
        let algorithm = hash_algorithm_from_name(&self.text[algorithm.span])
            .ok_or(UnknownHashFunction(algorithm.span))?;
        Ok(rito_hasher(width, algorithm))
    }
}
