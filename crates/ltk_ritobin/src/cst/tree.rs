use std::fmt::Display;

use ltk_meta::Bin;

use crate::{
    parse::{
        self, impls,
        tokenizer::{self, Token},
        ErrorPropagation, Parser, Span,
    },
    typecheck::visitor::DiagnosticWithSpan,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[rustfmt::skip]
pub enum Kind {
  ErrorTree,
  File, 
  TypeExpr, TypeArgList, TypeArg,
  Block, BlockKey, Class, ListItem, ListItemBlock,

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
            Self::ListItemBlock => "list item (block)",
            Self::Comment => "comment",
        })
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
/// The ritobin concrete syntax tree / a node in the syntax tree
///
/// See [`crate::cst`] for more information.
pub struct Cst {
    /// The span of this node in the source text
    pub span: Span,
    /// The type of this node
    pub kind: Kind,

    pub children: Vec<Child>,

    /// Parse errors - whether this contains the errors for its children depends on what
    /// [`ErrorPropagation`] the parser was using.
    #[cfg_attr(feature = "serde", serde(skip_deserializing))]
    pub errors: Vec<parse::Error>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
/// A [`Cst`] child node - either a [`Token`] or a [`Cst`]
pub enum Child {
    Token(Token),
    Tree(Cst),
}

impl Child {
    /// Get a reference to this node as a [`Token`], if it is one.
    pub fn token(&self) -> Option<&Token> {
        match self {
            Child::Token(token) => Some(token),
            Child::Tree(_) => None,
        }
    }

    /// Get a reference to this node as a [`Cst`], if it is one.
    pub fn tree(&self) -> Option<&Cst> {
        match self {
            Child::Token(_) => None,
            Child::Tree(cst) => Some(cst),
        }
    }

    /// The span of this node
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
    /// Parses a CST from ritobin source code.
    ///
    /// **NOTE:** Parsing errors will end up in [`Self::errors`] - make sure to check this if needed
    /// (e.g before calling [`Self::build_bin`] later)
    pub fn parse(text: &str) -> Self {
        Self::parse_with_config(text, ErrorPropagation::Move)
    }

    /// Parses a CST from ritobin source code, with definable error propagation behaviour.
    pub fn parse_with_config(text: &str, error_propagation: ErrorPropagation) -> Self {
        let tokens = tokenizer::lex(text);
        let mut p = Parser::new(text, tokens);
        impls::file(&mut p);
        p.build_tree(error_propagation)
    }

    /// Construct a best-effort [`Bin`] from this tree, returning any errors. If there are any
    /// errors returned, the [`Bin`] may only be partially constructed.
    pub fn build_bin(&self, text: &str) -> (Bin, Vec<DiagnosticWithSpan>) {
        let mut checker = crate::typecheck::visitor::TypeChecker::new(text);
        self.walk(&mut checker);
        checker.collect_to_bin()
    }

    /// Print this tree to a string for debugging purposes. This does **NOT** output ritobin, see [`crate::Print`] for
    /// actual ritobin pretty-printing.
    pub fn print(&self, buf: &mut String, level: usize, source: &str) {
        // let parent_indent = "│ ".repeat(level.saturating_sub(1));
        let parent_indent = "    ".repeat(level.saturating_sub(1));
        let indent = match level > 0 {
            true => "    ", // "├ "
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
                true => ' ',  // '└'
                false => ' ', // '├'
            };
            match child {
                Child::Token(token) => {
                    format_to!(
                        buf,
                        // "{parent_indent}│ {bar} {:?} ({:?})\n",
                        "{parent_indent}    {bar} {:?} ({:?})\n",
                        &source[token.span.start as _..token.span.end as _],
                        token.kind,
                    )
                }
                Child::Tree(tree) => tree.print(buf, level + 1, source),
            }
        }
        assert!(buf.ends_with('\n'));
    }
}
