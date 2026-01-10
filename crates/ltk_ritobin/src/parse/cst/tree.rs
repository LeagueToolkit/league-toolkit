use std::fmt::Display;

use crate::parse::{self, tokenizer::Token, Span};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[rustfmt::skip]
pub enum Kind {
  ErrorTree,
  File, 
  TypeExpr, TypeArgList, TypeArg,
  Block, BlockKey, Class, ListItem,

  Entry, EntryKey, EntryValue, EntryTerminator,
  Literal,

  Comment,
}
impl Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::ErrorTree => "error tree",
            Self::File => "file",
            Self::TypeExpr => "bin entry type",
            Self::TypeArgList => "type argument list",
            Self::TypeArg => "type argument",
            Self::Block => "block",
            Self::Entry => "bin entry",
            Self::EntryKey => "key",
            Self::EntryValue => "value",
            Self::Literal => "literal",
            Self::EntryTerminator => "bin entry terminator (new line or ';')",
            Self::BlockKey => "key",
            Self::Class => "bin class",
            Self::ListItem => "list item",
            Self::Comment => "comment",
        })
    }
}

#[derive(Clone, Debug)]
pub struct Cst {
    pub span: Span,
    pub kind: Kind,
    pub children: Vec<Child>,
    pub errors: Vec<parse::Error>,
}

#[derive(Clone, Debug)]
pub enum Child {
    Token(Token),
    Tree(Cst),
}

impl Child {
    pub fn token(&self) -> Option<&Token> {
        match self {
            Child::Token(token) => Some(token),
            Child::Tree(_) => None,
        }
    }
    pub fn tree(&self) -> Option<&Cst> {
        match self {
            Child::Token(_) => None,
            Child::Tree(cst) => Some(cst),
        }
    }
    pub fn span(&self) -> Span {
        match self {
            Child::Token(token) => token.span,
            Child::Tree(tree) => tree.span,
        }
    }
}

#[macro_export]
macro_rules! format_to {
    ($buf:expr) => ();
    ($buf:expr, $lit:literal $($arg:tt)*) => {
        { use ::std::fmt::Write as _; let _ = ::std::write!($buf, $lit $($arg)*); }
    };
}
impl Cst {
    pub fn print(&self, buf: &mut String, level: usize, source: &str) {
        let parent_indent = "│ ".repeat(level.saturating_sub(1));
        let indent = match level > 0 {
            true => "├ ",
            false => "",
        };
        let safe_span = match self.span.end >= self.span.start {
            true => &source[self.span],
            false => "!!!!!!",
        };
        format_to!(
            buf,
            "{parent_indent}{indent}{:?} - ({}..{}): {:?}\n",
            self.kind,
            self.span.start,
            self.span.end,
            safe_span
        );
        for (i, child) in self.children.iter().enumerate() {
            let bar = match i + 1 == self.children.len() {
                true => '└',
                false => '├',
            };
            match child {
                Child::Token(token) => {
                    format_to!(
                        buf,
                        "{parent_indent}│ {bar} {:?}\n",
                        &source[token.span.start as _..token.span.end as _]
                    )
                }
                Child::Tree(tree) => tree.print(buf, level + 1, source),
            }
        }
        assert!(buf.ends_with('\n'));
    }
}
