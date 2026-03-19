use std::{
    collections::VecDeque,
    fmt::{self, Write},
};

use crate::{
    cst::{visitor::Visit, Cst, Kind, Visitor},
    parse::TokenKind,
    print::{
        command::Mode,
        visitor::{CstVisitor, ListContext},
        PrintConfig, PrintError,
    },
    HashProvider,
};

impl<'a, W: fmt::Write, H: HashProvider> CstVisitor<'a, W, H> {
    fn enter_tree_inner(&mut self, tree: &Cst) -> Result<(), PrintError> {
        match tree.kind {
            Kind::TypeArgList => {
                let grp = self.begin_group(Some(Mode::Flat));
                // eprintln!("{:#?}", tree.children);
                self.list_stack.push(ListContext {
                    len: tree
                        .children
                        .iter()
                        .filter(|n| n.tree().is_some_and(|t| t.kind == Kind::TypeArg))
                        .count()
                        .try_into()
                        .unwrap(),
                    idx: 0,
                    grp,
                });
            }
            Kind::ListItemBlock => {
                self.softline();
                let grp = self.begin_group(None);

                let len = tree
                    .children
                    .iter()
                    .filter(|n| n.tree().is_some_and(|t| t.kind == Kind::ListItem))
                    .count();
                if len > 0 {
                    self.list_stack.push(ListContext {
                        len: len.try_into().unwrap(),
                        idx: 0,
                        grp,
                    });
                }
            }
            Kind::Block => {
                // eprintln!("BLOCK: {:#?}", tree.children);
                let grp = self.begin_group(None);
                let len = tree
                    .children
                    .iter()
                    .filter(|n| {
                        n.tree()
                            .is_some_and(|t| matches!(t.kind, Kind::ListItem | Kind::ListItemBlock))
                    })
                    .count();
                if len > 0 {
                    self.list_stack.push(ListContext {
                        len: len.try_into().unwrap(),
                        idx: 0,
                        grp,
                    });
                }
            }
            Kind::Class => {}
            Kind::ListItem => {
                if tree
                    .children
                    .first()
                    .is_some_and(|c| c.tree().is_some_and(|t| t.kind == Kind::Class))
                {
                    if let Some(list) = self.list_stack.last() {
                        self.force_group(list.grp, Mode::Break);
                    }
                }
                self.softline();
            }
            Kind::Entry => {
                self.line();
                // self.flush().unwrap();
            }
            _ => {}
        }
        Ok(())
    }

    fn exit_tree_inner(&mut self, tree: &Cst) -> Result<(), PrintError> {
        match tree.kind {
            Kind::TypeArgList => {
                self.list_stack.pop();
                self.end_group();
            }
            Kind::ListItemBlock | Kind::Block => {
                self.list_stack.pop();
                // eprintln!("exit {} | stack: {}", tree.kind, self.list_stack.len());
                if let Some(list) = self.list_stack.last() {
                    self.force_group(list.grp, Mode::Break);
                    self.softline();
                }
                self.end_group();
            }
            Kind::ListItem | Kind::TypeArg => {
                if let Some(ctx) = self.list_stack.last() {
                    let last_item = ctx.idx + 1 == ctx.len;

                    if !last_item {
                        self.text_if(",", Mode::Flat);
                        self.space();
                        if tree.kind == Kind::ListItem {
                            self.softline();
                        }
                    }

                    self.list_stack.last_mut().unwrap(/* guaranteed by if let */).idx += 1;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn visit_token_inner(
        &mut self,
        token: &crate::parse::Token,
        context: &Cst,
    ) -> Result<(), PrintError> {
        let txt = self.src[token.span].trim();
        let print_value = token.kind.print_value();

        if txt.is_empty() && print_value.is_none() {
            return Ok(());
        }

        // eprintln!("->{:?}", token.kind);
        match token.kind {
            TokenKind::LCurly => {
                self.space();
                self.text("{");
                self.indent(4);
                self.space();
                // self.softline();
            }

            TokenKind::RCurly => {
                self.dedent(4);
                self.softline();
                self.text("}");
            }

            TokenKind::Comma => {
                // self.text_if(",", Mode::Flat);
                // self.softline();
            }
            TokenKind::Colon => {
                self.text(":");
                self.space();
            }

            TokenKind::Eq => {
                self.space();
                self.text("=");
                self.space();
            }

            TokenKind::LBrack => {
                self.text("[");
            }
            TokenKind::RBrack => {
                self.text("]");
            }
            TokenKind::Quote => {
                self.text("\"");
            }
            TokenKind::False => {
                self.text("false");
            }
            TokenKind::True => {
                self.text("true");
            }

            _ => {
                self.text(txt);
            }
        }
        self.print_safe()?;
        // self.flush()?;
        Ok(())
    }
}

impl<'a, W: fmt::Write, H: HashProvider> Visitor for CstVisitor<'a, W, H> {
    fn enter_tree(&mut self, tree: &Cst) -> Visit {
        match self.enter_tree_inner(tree) {
            Ok(_) => Visit::Continue,
            Err(e) => {
                self.error.replace(e);
                Visit::Stop
            }
        }
    }
    fn exit_tree(&mut self, tree: &Cst) -> Visit {
        match self.exit_tree_inner(tree) {
            Ok(_) => Visit::Continue,
            Err(e) => {
                self.error.replace(e);
                Visit::Stop
            }
        }
    }
    fn visit_token(&mut self, token: &crate::parse::Token, context: &crate::cst::Cst) -> Visit {
        match self.visit_token_inner(token, context) {
            Ok(_) => Visit::Continue,
            Err(e) => {
                self.error.replace(e);
                Visit::Stop
            }
        }
    }
}
