use std::{
    fmt::{self, Display},
    ops::{Index, IndexMut},
};

use bumpalo::{collections, Bump};
use ltk_meta::Bin;

use crate::{
    cst::{
        visitor::{Visit, VisitCtx},
        Visitor,
    },
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

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug)]
/// The ritobin concrete syntax tree / a node in the syntax tree
///
/// See [`crate::cst`] for more information.
pub struct Cst<'arena> {
    pub(crate) nodes: bumpalo::collections::Vec<'arena, Node<'arena>>,
}

impl<'arena> Cst<'arena> {
    pub fn node(&self, id: NodeId) -> Option<&Node<'arena>> {
        self.nodes.get((id.0) as usize)
    }

    pub fn root(&self) -> &Node<'arena> {
        &self.nodes[0]
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct NodeId(pub(crate) u32);

impl<'arena> Index<NodeId> for [Node<'arena>] {
    type Output = Node<'arena>;

    fn index(&self, index: NodeId) -> &Self::Output {
        &self[index.0 as usize]
    }
}

impl<'arena> IndexMut<NodeId> for [Node<'arena>] {
    fn index_mut(&mut self, index: NodeId) -> &mut Self::Output {
        &mut self[index.0 as usize]
    }
}

impl<'arena> Index<NodeId> for collections::Vec<'arena, Node<'arena>> {
    type Output = Node<'arena>;

    fn index(&self, index: NodeId) -> &Self::Output {
        &self[index.0 as usize]
    }
}

impl<'arena> IndexMut<NodeId> for collections::Vec<'arena, Node<'arena>> {
    fn index_mut(&mut self, index: NodeId) -> &mut Self::Output {
        &mut self[index.0 as usize]
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug)]
pub struct Node<'arena> {
    /// The span of this node in the source text
    pub span: Span,
    /// The type of this node
    pub kind: Kind,
    pub children: bumpalo::collections::Vec<'arena, Child>,

    /// Parse errors - whether this contains the errors for its children depends on what
    /// [`ErrorPropagation`] the parser was using.
    #[cfg_attr(feature = "serde", serde(skip_deserializing))]
    pub errors: bumpalo::collections::Vec<'arena, parse::Error>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug)]
/// A [`Cst`] child node - either a [`Token`] or a [`Cst`]
pub enum Child {
    Token(Token),
    Tree(NodeId),
}

impl Child {
    /// Get a reference to this node as a [`Token`], if it is one.
    pub fn token(&self) -> Option<&Token> {
        match self {
            Child::Token(token) => Some(token),
            Child::Tree(_) => None,
        }
    }

    /// Get a reference to this node as a [`Node`], if it is one.
    pub fn tree<'arena: 'b, 'b>(&'b self, nodes: &'arena [Node]) -> Option<&'arena Node<'arena>> {
        match self {
            Child::Token(_) => None,
            Child::Tree(id) => Some(&nodes[id.0 as usize]),
        }
    }

    /// The span of this node
    pub fn span<'arena: 'b, 'b>(&'b self, nodes: &'arena [Node]) -> Span {
        match self {
            Child::Token(token) => token.span,
            Child::Tree(id) => nodes[id.0 as usize].span,
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
impl<'arena> Cst<'arena> {
    /// Parses a CST from ritobin source code.
    ///
    /// **NOTE:** Parsing errors will end up in [`Self::errors`] - make sure to check this if needed
    /// (e.g before calling [`Self::build_bin`] later)
    pub fn parse(arena: &'arena Bump, text: &str) -> Self {
        Self::parse_with_config(arena, text, ErrorPropagation::Move)
    }

    /// Parses a CST from ritobin source code, with definable error propagation behaviour.
    pub fn parse_with_config(
        arena: &'arena Bump,
        text: &str,
        error_propagation: ErrorPropagation,
    ) -> Self {
        let tokens = tokenizer::lex(text);
        let mut p = Parser::new(text, tokens);
        impls::file(&mut p);
        p.build_tree(arena, error_propagation)
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
    pub fn print(&self, mut buf: &mut String, source: &str) {
        let mut printer = DebugPrinter {
            writer: &mut buf,
            source,
            indent_level: 0,
        };
        self.walk(&mut printer);
    }
}

struct DebugPrinter<'b, 's, W: fmt::Write + ?Sized> {
    writer: &'b mut W,
    source: &'s str,
    indent_level: usize,
}

impl<W: fmt::Write + ?Sized> Visitor for DebugPrinter<'_, '_, W> {
    fn enter_tree(&mut self, ctx: &VisitCtx<'_>, tree: NodeId) -> Visit {
        let node = ctx.node(tree).unwrap();

        let indent = "    ".repeat(self.indent_level);

        let safe_span = if node.span.end >= node.span.start {
            &self.source[node.span]
        } else {
            "!!!!!!"
        };

        let _ = writeln!(
            self.writer,
            "{indent}{:?} - ({}..{}): {:?}",
            node.kind, node.span.start, node.span.end, safe_span
        );

        self.indent_level += 1;

        Visit::Continue
    }

    fn exit_tree(&mut self, _ctx: &VisitCtx<'_>, _tree: NodeId) -> Visit {
        self.indent_level -= 1;
        Visit::Continue
    }

    fn visit_token(&mut self, _ctx: &VisitCtx<'_>, token: Token, _parent: NodeId) -> Visit {
        let indent = "    ".repeat(self.indent_level);

        let text = &self.source[token.span.start as usize..token.span.end as usize];

        let _ = writeln!(self.writer, "{indent}{:?} ({:?})", text, token.kind);

        Visit::Continue
    }
}
