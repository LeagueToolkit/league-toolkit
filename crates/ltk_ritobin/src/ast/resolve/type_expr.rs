use ltk_meta::PropertyKind;

use crate::{
    ast::{
        builder::Builder,
        diagnostics::Diagnostic::{self, *},
        node::TypeExpr,
    },
    cst::{ChildrenExt as _, Kind},
    hash_algorithm_from_name, hash_width_from_name,
    parse::TokenKind,
    HashType, Node, RitoType, RitobinName as _,
};
use crate::{span::Span, ScalarType};

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
            return Ok(ScalarType::Hash(match args {
                Some(args) => self.resolve_hash_type(args)?,
                None => HashType::DEFAULT,
            })
            .into());
        }

        let mut hash_types = [HashType::DEFAULT; 2];
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
                                        kind: RitoType::from_property_kind(kind),
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
                                        kind: RitoType::from_property_kind(kind),
                                    }
                                    .unwrap(),
                                );
                            }
                            Some(_) => {}
                        }
                        if let Some(nested) = arg.find_tree(self.cst, Kind::TypeArgList) {
                            match resolved {
                                Some(PropertyKind::Hash) => match self.resolve_hash_type(nested) {
                                    Ok(hash_type) if i < hash_types.len() => {
                                        hash_types[i] = hash_type
                                    }
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

        Ok(RitoType::from_parts(base, subtypes).with_hash_types(hash_types))
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

    /// Resolves the type arguments `[width]` or `[width,algorithm]` of a `hash` type.
    ///
    /// # Errors
    /// Fails if the list does not have 1 or 2 arguments, if the first argument is not `u32` or
    /// `u64`, or if the second argument is not `fnv1a`, `xxh64` or `xxh3`.
    fn resolve_hash_type(&self, list: &Node) -> Result<HashType, Diagnostic> {
        let args = self.type_args(list);
        let (width, algorithm) = match args[..] {
            [width] => (width, None),
            [width, algorithm] => (width, Some(algorithm)),
            [] => {
                return Err(HashArgCountMismatch {
                    span: list.span,
                    got: 0,
                })
            }
            [_, _, first, ..] => {
                let last = args.last().copied().unwrap_or(first);
                return Err(HashArgCountMismatch {
                    span: Span::new(first.span.start, last.span.end),
                    got: args.len() as u8,
                });
            }
        };
        let width =
            hash_width_from_name(&self.text[width.span]).ok_or(UnknownHashWidth(width.span))?;
        let Some(algorithm) = algorithm else {
            return Ok(HashType::width_only(width));
        };
        let algorithm = hash_algorithm_from_name(&self.text[algorithm.span])
            .ok_or(UnknownHashFunction(algorithm.span))?;
        Ok(HashType::new(width, algorithm))
    }
}
