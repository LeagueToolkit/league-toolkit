use std::fmt::{self, Display};

use ltk_meta::Bin;

use crate::{
    cst::{
        visitor::{Visit, VisitCtx},
        ChildRange, ErrorRange, NodeId, TokenId, Visitor,
    },
    parse::{
        impls,
        tokenizer::{self, Token},
        Error, ErrorPropagation, Parser, Span,
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
pub struct Cst {
    pub(crate) nodes: Vec<Node>,
    pub(crate) children: Vec<Child>,
    pub(crate) tokens: Vec<Token>,
    pub errors: Vec<Error>,
}

impl Cst {
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get((id.0) as usize)
    }
    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut((id.0) as usize)
    }

    pub fn token(&self, id: TokenId) -> Option<&Token> {
        self.tokens.get((id.0) as usize)
    }
    pub fn token_mut(&mut self, id: TokenId) -> Option<&mut Token> {
        self.tokens.get_mut((id.0) as usize)
    }

    pub(crate) fn push_node(&mut self, node: Node) -> NodeId {
        let id = NodeId(self.nodes.len().try_into().unwrap());
        self.nodes.push(node);
        id
    }
    pub(crate) fn push_token(&mut self, token: Token) -> TokenId {
        let id = TokenId(self.tokens.len().try_into().unwrap());
        self.tokens.push(token);
        id
    }
    pub(crate) fn push_children(
        &mut self,
        children: impl IntoIterator<Item = Child>,
    ) -> ChildRange {
        let start = u32::try_from(self.children.len()).unwrap();
        self.children.extend(children);
        let end = u32::try_from(self.children.len()).unwrap();
        ChildRange {
            start,
            len: end - start,
        }
    }
    pub(crate) fn push_errors(&mut self, errors: impl IntoIterator<Item = Error>) -> ErrorRange {
        let start = u32::try_from(self.children.len()).unwrap();
        self.errors.extend(errors);
        let end = u32::try_from(self.children.len()).unwrap();
        ErrorRange {
            start,
            len: end - start,
        }
    }

    pub fn root(&self) -> &Node {
        &self.nodes[0]
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug)]
pub struct Node {
    /// The span of this node in the source text
    pub span: Span,
    /// The type of this node
    pub kind: Kind,
    pub children: ChildRange,

    /// Parse errors - whether this contains the errors for its children depends on what
    /// [`ErrorPropagation`] the parser was using.
    #[cfg_attr(feature = "serde", serde(skip_deserializing))]
    pub errors: ErrorRange,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug)]
/// A [`Cst`] child node - either a [`Token`] or a [`Cst`]
pub enum Child {
    Token(TokenId),
    Tree(NodeId),
}

impl Child {
    /// Get a reference to this node as a [`Token`], if it is one.
    pub fn token<'a>(&'a self, cst: &'a Cst) -> Option<&'a Token> {
        match self {
            Child::Token(id) => cst.token(*id),
            Child::Tree(_) => None,
        }
    }

    /// Get a reference to this node as a [`Node`], if it is one.
    pub fn tree<'a>(&'a self, cst: &'a Cst) -> Option<&'a Node> {
        match self {
            Child::Token(_) => None,
            Child::Tree(id) => cst.node(*id),
        }
    }

    /// The span of this node
    pub fn span(&self, cst: &Cst) -> Span {
        match self {
            Child::Token(id) => cst.token(*id).unwrap().span,
            Child::Tree(id) => cst.node(*id).unwrap().span,
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

    fn visit_token(&mut self, ctx: &VisitCtx<'_>, token: TokenId, _parent: NodeId) -> Visit {
        let token = ctx.cst.token(token).unwrap();

        let indent = "    ".repeat(self.indent_level);

        let text = &self.source[token.span.start as usize..token.span.end as usize];

        let _ = writeln!(self.writer, "{indent}{:?} ({:?})", text, token.kind);

        Visit::Continue
    }
}
