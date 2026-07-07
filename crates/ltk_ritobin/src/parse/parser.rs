use std::cell::Cell;

use bumpalo::{collections, Bump};

use crate::{
    cst::{Node, NodeId},
    parse::{
        cst::{Child, Cst, Kind as TreeKind},
        error::{Error, ErrorKind},
        tokenizer::{Token, TokenKind},
        Span,
    },
};

#[derive(Debug)]
pub enum Event {
    Open { kind: TreeKind },
    Close,
    Error { kind: ErrorKind, span: Option<Span> },
    Advance,
}

#[derive(Clone, Copy)]
pub struct MarkOpened {
    index: usize,
}

#[derive(Clone, Copy)]
pub struct MarkClosed {
    index: usize,
}

/// The ritobin parser.
/// You should **NOT** use this directly unless you know what you're doing - see
/// [`Cst::parse`]
pub struct Parser<'a> {
    pub text: &'a str,
    pub tokens: Vec<Token>,
    pos: usize,
    fuel: Cell<u32>,
    pub events: Vec<Event>,
}

/// How [`Cst`] nodes in the tree should propagate their errors. See [`Cst::parse_with_config`]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum ErrorPropagation {
    /// No propagation, errors will remain in their respective nodes
    None,
    #[default]
    /// Child nodes will move their errors to their parent. This is the default, since it
    /// conveniently accumulates all parse errors to the root node.
    Move,
    /// Child nodes will copy their errors to their parent.
    Clone,
}

impl<'a> Parser<'a> {
    pub fn new(text: &'a str, tokens: Vec<Token>) -> Parser<'a> {
        Parser {
            text,
            tokens,
            pos: 0,
            fuel: Cell::new(256),
            events: Vec::new(),
        }
    }

    pub fn build_tree<'arena>(
        self,
        arena: &'arena Bump,
        error_propagation: ErrorPropagation,
    ) -> Cst<'arena> {
        let last_token = self.tokens.last().copied();

        let mut tokens = self.tokens.into_iter().peekable();
        let mut events = self.events;

        assert!(matches!(events.pop(), Some(Event::Close)));
        let mut nodes = collections::Vec::new_in(arena);
        let mut stack = Vec::new();
        let mut last_span = Span::default();
        let mut just_opened = false;
        for event in events.into_iter() {
            match event {
                Event::Open { kind } => {
                    just_opened = true;

                    let idx = NodeId(nodes.len().try_into().unwrap());
                    nodes.push(Node {
                        span: Span::new(last_span.end, 0),
                        kind,
                        children: collections::Vec::new_in(arena),
                        errors: collections::Vec::new_in(arena),
                    });

                    stack.push(idx);
                }
                Event::Close => {
                    let node_idx = stack.pop().unwrap();
                    let parent_idx = *stack.last().unwrap();

                    let (parent, node) = {
                        let (left, right) = nodes.split_at_mut(node_idx.0 as usize);
                        (&mut left[parent_idx], &mut right[0])
                    };

                    if node.span.end == 0 {
                        // empty trees
                        node.span.end = node.span.start;
                    }

                    parent.span.end = node.span.end.max(parent.span.end); // update our parent tree's span
                    parent.children.push(Child::Tree(node_idx));
                    match error_propagation {
                        ErrorPropagation::None => {}
                        ErrorPropagation::Move => parent.errors.append(&mut node.errors),
                        ErrorPropagation::Clone => parent.errors.extend_from_slice(&node.errors),
                    }
                }
                Event::Advance => {
                    let token = tokens.next().unwrap();
                    let last_idx = *stack.last().unwrap();
                    let last = &mut nodes[last_idx];

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
                    let cur_tree_idx = *stack.last().unwrap();
                    let cur_tree = &nodes[cur_tree_idx];

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
                                .or(cur_tree.children.last().map(|c| c.span(&nodes)))
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

                    let cur_tree = &mut nodes[cur_tree_idx];
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
        assert_eq!(tree, NodeId(0));
        Cst { nodes }
    }

    pub(crate) fn open(&mut self) -> MarkOpened {
        fn open(p: &mut Parser<'_>) -> MarkOpened {
            let mark = MarkOpened {
                index: p.events.len(),
            };
            p.events.push(Event::Open {
                kind: TreeKind::ErrorTree,
            });
            mark
        }

        let m = open(self);
        if self.events.len() == 1 {
            return m;
        }
        let mut had_comments = false;
        while self.eat(TokenKind::Comment) {
            had_comments = true;
        }
        if had_comments {
            self.close(m, TreeKind::Comment);
            return open(self);
        }
        m
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

    #[inline]
    pub(crate) fn nth(&self, lookahead: usize) -> TokenKind {
        #[cold]
        #[inline(never)]
        fn stuck(p: &Parser<'_>) -> ! {
            eprintln!("last 5 tokens behind self.pos:");
            for tok in &p.tokens[p.pos.saturating_sub(5)..p.pos] {
                eprintln!(" - {:?}: {:?}", tok.kind, &p.text[tok.span]);
            }

            panic!("parser is stuck")
        }

        match self.fuel.get() {
            0 => stuck(self),
            f => self.fuel.set(f - 1),
        }

        self.tokens
            .get(self.pos + lookahead)
            .map_or(TokenKind::Eof, |it| it.kind)
    }

    #[inline]
    pub(crate) fn at(&self, kind: TokenKind) -> bool {
        self.nth(0) == kind
    }

    #[inline]
    pub(crate) fn at_any(&self, kinds: &[TokenKind]) -> Option<TokenKind> {
        let kind = self.nth(0);
        kinds.contains(&kind).then_some(kind)
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

    #[allow(unused)]
    pub(crate) fn expect_fallable(&mut self, kind: TokenKind) -> Result<(), ()> {
        match self.expect(kind) {
            true => Ok(()),
            false => Err(()),
        }
    }
}
