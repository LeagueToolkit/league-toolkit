use std::{cell::Cell, marker::PhantomData};

use bumpalo::{collections, Bump};
use smallvec::SmallVec;

use crate::{
    cst::{ChildRange, ErrorRange, Node, NodeId},
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

struct StackItem {
    idx: NodeId,
    children: SmallVec<[Child; 4]>,
    errors: SmallVec<[Error; 1]>,
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

        let nodes_len = events
            .iter()
            .filter(|e| matches!(e, Event::Open { .. }))
            .count();

        let mut cst = Cst {
            nodes: Vec::with_capacity(nodes_len),
            children: Vec::with_capacity(nodes_len),
            tokens: vec![],
            errors: vec![],
        };

        let mut stack = Vec::new();
        let mut last_span = Span::default();
        let mut just_opened = false;
        for event in events.into_iter() {
            match event {
                Event::Open { kind } => {
                    just_opened = true;

                    stack.push(StackItem {
                        idx: cst.push_node(Node {
                            span: Span::new(last_span.end, 0),
                            kind,
                            children: ChildRange::empty(),
                            errors: ErrorRange::empty(),
                            phantom: PhantomData,
                        }),
                        children: SmallVec::new(),
                        errors: SmallVec::new(),
                    });
                }
                Event::Close => {
                    let mut cur = stack.pop().unwrap();
                    cst.node_mut(cur.idx).unwrap().children = cst.push_children(cur.children);

                    let parent = stack.last_mut().unwrap();

                    {
                        let (parent_node, cur_node) = {
                            let (left, right) = cst.nodes.split_at_mut(cur.idx.0 as usize);
                            (&mut left[parent.idx], &mut right[0])
                        };

                        if cur_node.span.end == 0 {
                            // empty trees
                            cur_node.span.end = cur_node.span.start;
                        }

                        parent_node.span.end = cur_node.span.end.max(parent_node.span.end); // update our parent tree's span
                        parent.children.push(Child::Tree(cur.idx));

                        match error_propagation {
                            ErrorPropagation::None => {}
                            ErrorPropagation::Move => parent.errors.append(&mut cur.errors),
                            ErrorPropagation::Clone => parent.errors.extend_from_slice(&cur.errors),
                        }
                    }

                    cst.node_mut(cur.idx).unwrap().errors = cst.push_errors(cur.errors);
                }
                Event::Advance => {
                    let token = tokens.next().unwrap();
                    let last = stack.last_mut().unwrap();
                    {
                        let last = cst.node_mut(last.idx).unwrap();

                        if just_opened {
                            // first token of the tree
                            last.span.start = token.span.start;
                        }
                        just_opened = false;

                        last.span.end = token.span.end;
                        last_span = token.span;
                    }
                    let token = Child::Token(cst.push_token(token));
                    last.children.push(token);
                }
                Event::Error { kind, span } => {
                    let cur = stack.last_mut().unwrap();
                    let cur_node = cst.node(cur.idx).unwrap();

                    let span = match cur_node.kind {
                        TreeKind::ErrorTree => cur_node.span,
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
                            ErrorKind::UnexpectedTree => cur_node.span,
                            _ => span
                                .or(cur.children.last().map(|c| c.span(&cst)))
                                // we can't use Tree.span.end because that's only known on Close
                                .unwrap_or(Span::new(
                                    cur_node.span.start,
                                    last_token
                                        .as_ref()
                                        .map(|t| t.span.end)
                                        .unwrap_or(cur_node.span.start),
                                )),
                        },
                    };

                    cur.errors.push(Error {
                        span,
                        tree: cur_node.kind,
                        kind,
                    });
                }
            }
        }

        let root = stack.pop().unwrap();
        {
            let children = cst.push_children(root.children);
            let errors = cst.push_errors(root.errors);

            let root_node = cst.node_mut(root.idx).unwrap();
            root_node.children = children;
            root_node.errors = errors;
        }

        assert!(stack.is_empty());
        assert!(tokens.next().is_none());
        assert_eq!(root.idx, NodeId(0));
        cst
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
