use std::cell::Cell;

use crate::parse::{
    cst::{Child, Cst, Kind as TreeKind},
    error::{Error, ErrorKind},
    tokenizer::{Token, TokenKind},
    Span,
};

pub enum Event {
    Open { kind: TreeKind },
    Close,
    Error { kind: ErrorKind, span: Option<Span> },
    Advance,
}

pub struct MarkOpened {
    index: usize,
}
pub struct MarkClosed {
    index: usize,
}

pub struct Parser<'a> {
    pub text: &'a str,
    pub tokens: Vec<Token>,
    pos: usize,
    fuel: Cell<u32>,
    pub events: Vec<Event>,
}

impl<'a> Parser<'a> {
    pub fn new(text: &'a str, tokens: Vec<Token>) -> Parser {
        Parser {
            text,
            tokens,
            pos: 0,
            fuel: Cell::new(256),
            events: Vec::new(),
        }
    }

    pub fn build_tree(self) -> Cst {
        let last_token = self.tokens.last().copied();

        let mut tokens = self.tokens.into_iter().peekable();
        let mut events = self.events;

        assert!(matches!(events.pop(), Some(Event::Close)));
        let mut stack = Vec::new();
        let mut last_span = Span::default();
        let mut just_opened = false;
        for event in events {
            match event {
                Event::Open { kind } => {
                    just_opened = true;
                    stack.push(Cst {
                        span: Span::new(last_span.end, 0),
                        kind,
                        children: Vec::new(),
                        errors: Vec::new(),
                    })
                }
                Event::Close => {
                    let mut tree = stack.pop().unwrap();
                    let last = stack.last_mut().unwrap();
                    if tree.span.end == 0 {
                        // empty trees
                        tree.span.end = tree.span.start;
                    }
                    last.span.end = tree.span.end.max(last.span.end); // update our parent tree's span
                    last.children.push(Child::Tree(tree));
                }
                Event::Advance => {
                    let token = tokens.next().unwrap();
                    let last = stack.last_mut().unwrap();

                    if just_opened {
                        // first token of the tree
                        last.span.start = token.span.start;
                    }
                    just_opened = false;

                    last.span.end = token.span.end;
                    last_span = token.span;
                    last.children.push(Child::Token(token));
                }
                Event::Error { kind, span } => {
                    let cur_tree = stack.last_mut().unwrap();
                    let span = match cur_tree.kind {
                        TreeKind::ErrorTree => cur_tree.span,
                        _ => match kind {
                            // these errors are talking about what they wanted next
                            ErrorKind::Expected { .. } | ErrorKind::Unexpected { .. } => {
                                let mut span = tokens.peek().map(|t| t.span).unwrap_or(last_span);
                                // so we point at the character just after our token
                                span.end += 1;
                                span.start = span.end - 1;
                                span
                            }
                            // whole tree is the problem
                            ErrorKind::UnexpectedTree => cur_tree.span,
                            _ => span
                                .or(cur_tree.children.last().map(|c| c.span()))
                                // we can't use Tree.span.end because that's only known on Close
                                .unwrap_or(Span::new(
                                    cur_tree.span.start,
                                    last_token
                                        .as_ref()
                                        .map(|t| t.span.end)
                                        .unwrap_or(cur_tree.span.start),
                                )),
                        },
                    };
                    cur_tree.errors.push(Error {
                        span,
                        tree: cur_tree.kind,
                        kind,
                    });
                }
            }
        }

        let tree = stack.pop().unwrap();
        assert!(stack.is_empty());
        assert!(tokens.next().is_none());
        tree
    }

    pub(crate) fn open(&mut self) -> MarkOpened {
        let mark = MarkOpened {
            index: self.events.len(),
        };
        self.events.push(Event::Open {
            kind: TreeKind::ErrorTree,
        });
        mark
    }

    pub(crate) fn scope<F, R>(&mut self, kind: TreeKind, mut f: F) -> (R, MarkClosed)
    where
        F: FnMut(&mut Self) -> R,
    {
        let m = MarkOpened {
            index: self.events.len(),
        };
        self.events.push(Event::Open {
            kind: TreeKind::ErrorTree,
        });
        let ret = f(self);
        self.events[m.index] = Event::Open { kind };
        self.events.push(Event::Close);
        (ret, MarkClosed { index: m.index })
    }

    pub(crate) fn open_before(&mut self, m: MarkClosed) -> MarkOpened {
        let mark = MarkOpened { index: m.index };
        self.events.insert(
            m.index,
            Event::Open {
                kind: TreeKind::ErrorTree,
            },
        );
        mark
    }

    pub(crate) fn close(&mut self, m: MarkOpened, kind: TreeKind) -> MarkClosed {
        self.events[m.index] = Event::Open { kind };
        self.events.push(Event::Close);
        MarkClosed { index: m.index }
    }

    pub(crate) fn advance(&mut self) {
        assert!(!self.eof());
        self.fuel.set(256);
        self.events.push(Event::Advance);
        self.pos += 1;
    }

    pub(crate) fn report(&mut self, kind: ErrorKind) {
        self.events.push(Event::Error { kind, span: None });
    }

    pub(crate) fn advance_with_error(&mut self, kind: ErrorKind, span: Option<Span>) {
        let m = self.open();
        self.advance();
        self.events.push(Event::Error { kind, span });
        self.close(m, TreeKind::ErrorTree);
    }

    pub(crate) fn eof(&self) -> bool {
        self.pos == self.tokens.len()
    }

    pub(crate) fn nth(&self, lookahead: usize) -> TokenKind {
        if self.fuel.get() == 0 {
            eprintln!("last 5 tokens behind self.pos:");
            for tok in &self.tokens[self.pos.saturating_sub(5)..self.pos] {
                eprintln!(" - {:?}: {:?}", tok.kind, &self.text[tok.span]);
            }

            panic!("parser is stuck")
        }
        self.fuel.set(self.fuel.get() - 1);
        self.tokens
            .get(self.pos + lookahead)
            .map_or(TokenKind::Eof, |it| it.kind)
    }

    pub(crate) fn at(&self, kind: TokenKind) -> bool {
        self.nth(0) == kind
    }

    pub(crate) fn at_any(&self, kinds: &[TokenKind]) -> Option<TokenKind> {
        kinds.contains(&self.nth(0)).then_some(self.nth(0))
    }

    pub(crate) fn eat_any(&mut self, kinds: &[TokenKind]) -> Option<TokenKind> {
        if let Some(kind) = self.at_any(kinds) {
            self.advance();
            Some(kind)
        } else {
            None
        }
    }

    pub(crate) fn eat(&mut self, kind: TokenKind) -> bool {
        if self.at(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub(crate) fn expect_any(&mut self, kinds: &'static [TokenKind]) -> Option<TokenKind> {
        if let Some(kind) = self.eat_any(kinds) {
            return Some(kind);
        }
        self.report(ErrorKind::ExpectedAny {
            expected: kinds,
            got: self.nth(0),
        });
        None
    }

    pub(crate) fn expect(&mut self, kind: TokenKind) -> bool {
        if self.eat(kind) {
            return true;
        }
        self.report(ErrorKind::Expected {
            expected: kind,
            got: self.nth(0),
        });
        false
    }

    pub(crate) fn expect_fallable(&mut self, kind: TokenKind) -> Result<(), ()> {
        match self.expect(kind) {
            true => Ok(()),
            false => Err(()),
        }
    }
}
