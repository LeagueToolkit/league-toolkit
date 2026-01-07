use std::{
    cell::Cell,
    fmt,
    sync::{Arc, Mutex},
};

use super::tokenizer::{lex, Token, TokenKind};

#[salsa::db]
#[derive(Clone)]
#[cfg_attr(not(test), derive(Default))]
pub struct CalcDatabaseImpl {
    storage: salsa::Storage<Self>,

    // The logs are only used for testing and demonstrating reuse:
    #[cfg(test)]
    logs: Arc<Mutex<Option<Vec<String>>>>,
}

#[cfg(test)]
impl Default for CalcDatabaseImpl {
    fn default() -> Self {
        let logs = <Arc<Mutex<Option<Vec<String>>>>>::default();
        Self {
            storage: salsa::Storage::new(Some(Box::new({
                let logs = logs.clone();
                move |event| {
                    eprintln!("Event: {event:?}");
                    // Log interesting events, if logging is enabled
                    if let Some(logs) = &mut *logs.lock().unwrap() {
                        // only log interesting events
                        if let salsa::EventKind::WillExecute { .. } = event.kind {
                            logs.push(format!("Event: {event:?}"));
                        }
                    }
                }
            }))),
            logs,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Visit {
    Stop,
    /// Skip the current tree
    Skip,
    Continue,
}
#[allow(unused_variables)]
pub trait Visitor {
    #[must_use]
    fn enter_tree(&mut self, kind: TreeKind) -> Visit {
        Visit::Continue
    }
    #[must_use]
    fn exit_tree(&mut self, kind: TreeKind) -> Visit {
        Visit::Continue
    }
    #[must_use]
    fn visit_token(&mut self, token: &Token, context: TreeKind) -> Visit {
        Visit::Continue
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[rustfmt::skip]
pub enum TreeKind {
  ErrorTree,
  File, 
  TypeExpr, TypeArgList, TypeArg,
  Block,
  StmtEntry,
  ExprLiteral, ExprName,
}

#[derive(Clone)]
pub struct Tree {
    kind: TreeKind,
    children: Vec<Child>,
}

impl Tree {
    pub fn walk<V: Visitor>(&self, visitor: &mut V) {
        self.walk_inner(visitor);
    }

    fn walk_inner<V: Visitor>(&self, visitor: &mut V) -> Visit {
        let enter = visitor.enter_tree(self.kind);
        if matches!(enter, Visit::Stop | Visit::Skip) {
            return enter;
        }

        for child in &self.children {
            match child {
                Child::Token(token) => match visitor.visit_token(token, self.kind) {
                    Visit::Continue => {}
                    Visit::Skip => break,
                    Visit::Stop => return Visit::Stop,
                },
                Child::Tree(child_tree) => match child_tree.walk_inner(visitor) {
                    Visit::Continue => {}
                    Visit::Skip => {
                        break;
                    }
                    Visit::Stop => return Visit::Stop,
                },
            }
        }

        visitor.exit_tree(self.kind)
    }
}

#[derive(Clone)]
pub enum Child {
    Token(Token),
    Tree(Tree),
}

pub enum Event {
    Open { kind: TreeKind },
    Close,
    Advance,
}

pub struct MarkOpened {
    index: usize,
}
pub struct MarkClosed {
    index: usize,
}

pub struct Parser {
    pub tokens: Vec<Token>,
    pos: usize,
    fuel: Cell<u32>,
    pub events: Vec<Event>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Parser {
        Parser {
            tokens,
            pos: 0,
            fuel: Cell::new(256),
            events: Vec::new(),
        }
    }

    pub fn build_tree(self) -> Tree {
        let mut tokens = self.tokens.into_iter();
        let mut events = self.events;

        assert!(matches!(events.pop(), Some(Event::Close)));
        let mut stack = Vec::new();
        for event in events {
            match event {
                Event::Open { kind } => stack.push(Tree {
                    kind,
                    children: Vec::new(),
                }),
                Event::Close => {
                    let tree = stack.pop().unwrap();
                    stack.last_mut().unwrap().children.push(Child::Tree(tree));
                }
                Event::Advance => {
                    let token = tokens.next().unwrap();
                    stack.last_mut().unwrap().children.push(Child::Token(token));
                }
            }
        }

        let tree = stack.pop().unwrap();
        assert!(stack.is_empty());
        assert!(tokens.next().is_none());
        tree
    }

    fn open(&mut self) -> MarkOpened {
        let mark = MarkOpened {
            index: self.events.len(),
        };
        self.events.push(Event::Open {
            kind: TreeKind::ErrorTree,
        });
        mark
    }

    fn open_before(&mut self, m: MarkClosed) -> MarkOpened {
        let mark = MarkOpened { index: m.index };
        self.events.insert(
            m.index,
            Event::Open {
                kind: TreeKind::ErrorTree,
            },
        );
        mark
    }

    fn close(&mut self, m: MarkOpened, kind: TreeKind) -> MarkClosed {
        self.events[m.index] = Event::Open { kind };
        self.events.push(Event::Close);
        MarkClosed { index: m.index }
    }

    fn advance(&mut self) {
        assert!(!self.eof());
        self.fuel.set(256);
        self.events.push(Event::Advance);
        self.pos += 1;
    }

    fn advance_with_error(&mut self, error: &str) {
        let m = self.open();
        // TODO: Error reporting.
        eprintln!("{error}");
        self.advance();
        self.close(m, TreeKind::ErrorTree);
    }

    fn eof(&self) -> bool {
        self.pos == self.tokens.len()
    }

    fn nth(&self, lookahead: usize) -> TokenKind {
        if self.fuel.get() == 0 {
            panic!("parser is stuck")
        }
        self.fuel.set(self.fuel.get() - 1);
        self.tokens
            .get(self.pos + lookahead)
            .map_or(TokenKind::Eof, |it| it.kind)
    }

    fn at(&self, kind: TokenKind) -> bool {
        self.nth(0) == kind
    }

    fn at_any(&self, kinds: &[TokenKind]) -> bool {
        kinds.contains(&self.nth(0))
    }

    fn eat(&mut self, kind: TokenKind) -> bool {
        if self.at(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: TokenKind) {
        if self.eat(kind) {
            return;
        }
        // TODO: Error reporting.
        eprintln!("expected {kind:?}");
    }
}

pub fn parse(text: &str) -> Tree {
    let tokens = lex(text);
    eprintln!("tokens: {tokens:#?}");
    let mut p = Parser::new(tokens);
    file(&mut p);
    p.build_tree()
}

pub fn file(p: &mut Parser) {
    let m = p.open();
    while !p.eof() {
        if p.at(TokenKind::Name) {
            stmt_entry(p)
        } else {
            p.advance_with_error("expected a name");
        }
    }
    p.close(m, TreeKind::File);
}
pub fn stmt_entry(p: &mut Parser) {
    let m = p.open();

    p.expect(TokenKind::Name);
    p.expect(TokenKind::Colon);
    expr_type(p);
    p.expect(TokenKind::Eq);
    match p.nth(0) {
        TokenKind::Quote => {
            let m = p.open();
            p.advance();
            p.expect(TokenKind::String);
            p.expect(TokenKind::Quote);
            p.close(m, TreeKind::ExprLiteral);
        }
        TokenKind::Int | TokenKind::Minus => {
            let m = p.open();
            p.advance();
            p.close(m, TreeKind::ExprLiteral);
        }
        TokenKind::LCurly => {
            block(p);
        }
        TokenKind::Eof | TokenKind::Name => {}
        _ => p.advance_with_error("unexpected"),
    }

    p.close(m, TreeKind::StmtEntry);
}

pub fn expr_type(p: &mut Parser) -> MarkClosed {
    let m = p.open();
    p.expect(TokenKind::Name);
    if p.eat(TokenKind::LBrack) {
        while !p.at(TokenKind::RBrack) && !p.eof() {
            if p.at(TokenKind::Name) {
                expr_type_arg(p);
            } else {
                break;
            }
        }
        p.expect(TokenKind::RBrack);
    }
    p.close(m, TreeKind::TypeExpr)
}

pub fn expr_type_arg(p: &mut Parser) -> MarkClosed {
    use TokenKind::*;
    assert!(p.at(Name));
    let m = p.open();

    p.expect(Name);
    if !p.at(RParen) {
        p.expect(Comma);
    }

    p.close(m, TreeKind::TypeArg)
}

pub fn block(p: &mut Parser) {
    use TokenKind::*;
    assert!(p.at(LCurly));
    let m = p.open();

    p.expect(LCurly);
    while !p.at(RCurly) && !p.eof() {
        stmt_entry(p)
    }
    p.expect(RCurly);

    p.close(m, TreeKind::Block);
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn smoke_test() {
        let text = r#"
type = 4 
version: list[u32, = 3
linked: list[string] = {
    "DATA/Characters/Test/Animations/Skin0.bin\"
    "DATA/Characters/Test/Test.bin"

version: u32 = 5

"#;
        let cst = parse(text);
        let mut str = String::new();
        cst.print(&mut str, 0, text);
        eprintln!("{str}");
    }
}

#[macro_export]
macro_rules! format_to {
    ($buf:expr) => ();
    ($buf:expr, $lit:literal $($arg:tt)*) => {
        { use ::std::fmt::Write as _; let _ = ::std::write!($buf, $lit $($arg)*); }
    };
}
impl Tree {
    fn print(&self, buf: &mut String, level: usize, source: &str) {
        let parent_indent = "│ ".repeat(level.saturating_sub(1));
        let indent = match level > 0 {
            true => "├ ",
            false => "",
        };
        format_to!(buf, "{parent_indent}{indent}{:?}\n", self.kind);
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

#[salsa::db]
impl salsa::Database for CalcDatabaseImpl {}

#[salsa::input(debug)]
pub struct SourceProgram {
    #[returns(ref)]
    pub text: String,
}

#[salsa::tracked(debug)]
pub struct RitobinFile<'db> {
    #[tracked]
    #[returns(ref)]
    pub statements: Vec<Statement<'db>>,
}

#[salsa::interned(debug)]
pub struct PropertyName<'db> {
    #[returns(ref)]
    pub text: String,
}

#[derive(PartialEq, Debug, Hash, salsa::Update)]
pub struct Statement<'db> {
    pub span: Span<'db>,
    pub name: PropertyName<'db>,
    pub value: BinProperty,
}

#[salsa::accumulator]
#[derive(Debug)]
#[allow(dead_code)] // Debug impl uses them
pub struct Diagnostic {
    pub start: usize,
    pub end: usize,
    pub message: String,
}
impl Diagnostic {
    pub fn new(start: usize, end: usize, message: String) -> Self {
        Diagnostic {
            start,
            end,
            message,
        }
    }

    // #[cfg(test)]
    // pub fn render(&self, db: &dyn crate::Db, src: SourceProgram) -> String {
    //     use annotate_snippets::*;
    //     let line_start = src.text(db)[..self.start].lines().count() + 1;
    //     Renderer::plain()
    //         .render(
    //             Level::Error.title(&self.message).snippet(
    //                 Snippet::source(src.text(db))
    //                     .line_start(line_start)
    //                     .origin("input")
    //                     .fold(true)
    //                     .annotation(Level::Error.span(self.start..self.end).label("here")),
    //             ),
    //         )
    //         .to_string()
    // }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, PartialEq, Debug, Hash, salsa::Update)]
pub struct BinProperty {
    pub name_hash: u32,
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub value: ltk_meta::PropertyValueEnum,
}

#[salsa::tracked(debug)]
pub struct Span<'db> {
    #[tracked]
    pub start: usize,
    #[tracked]
    pub end: usize,
}
