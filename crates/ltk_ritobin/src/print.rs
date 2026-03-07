use std::{
    cell::RefCell,
    fmt::{self},
};

use crate::parse::{
    cst::{visitor::Visit, Cst, Kind, Visitor},
    Span, TokenKind,
};

#[derive(Debug, thiserror::Error)]
pub enum PrintError {
    #[error(transparent)]
    FmtError(#[from] fmt::Error),
}

#[derive(Default)]
struct Flags {
    break_pre: bool,
    break_post: bool,
    space_pre: bool,
    space_post: bool,
}

pub struct Printer<'a, W: fmt::Write> {
    source: &'a str,
    out: &'a mut W,
    error: Option<PrintError>,
    indent: u32,
    flags: Flags,
    default_flags: Flags,
}

impl<'a, W: fmt::Write> Printer<'a, W> {
    pub fn new(source: &'a str, dest: &'a mut W) -> Self {
        Self {
            source,
            out: dest,
            error: None,
            indent: 0,
            flags: Flags::default(),
            default_flags: Flags::default(),
        }
    }

    pub fn print(mut self, cst: &Cst) -> Result<(), PrintError> {
        cst.walk(&mut self);
        match self.error.take() {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    fn visit_token_inner(
        &mut self,
        token: &crate::parse::Token,
        context: &crate::parse::cst::Cst,
    ) -> Result<(), PrintError> {
        let token_txt = self.source[token.span].trim();
        if token_txt.is_empty() {
            return Ok(());
        }

        match token.kind {
            TokenKind::RCurly => {
                self.indent -= 1;
                self.flags.break_pre = true;
            }
            TokenKind::LCurly => {
                self.flags.space_pre = true;
                self.flags.break_post = true;
            }
            TokenKind::Colon => {
                self.flags.space_post = true;
            }
            TokenKind::Eq => {
                self.flags.space_pre = true;
                self.flags.space_post = true;
            }
            TokenKind::Comma => {
                self.flags.break_post = self.flags.break_pre;
                self.flags.break_pre = false;
                self.flags.space_post = true;
            }
            _ => {
                eprintln!("{:?}", context.kind);
            }
        }

        if self.flags.break_pre {
            self.out.write_char('\n')?;
            for _ in 0..self.indent {
                self.out.write_str("    ")?;
            }
        } else if self.flags.space_pre {
            self.out.write_char(' ')?;
        }
        self.out.write_str(token_txt)?;

        self.flags.break_pre = self.flags.break_post;
        self.flags.break_post = false;

        self.flags.space_pre = self.flags.space_post;
        self.flags.space_post = false;

        if token.kind == TokenKind::LCurly {
            self.indent += 1;
        }

        Ok(())
    }
}

