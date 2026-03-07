use std::{
    collections::VecDeque,
    fmt::{self, Write},
};

use crate::{
    parse::{
        cst::{visitor::Visit, Cst, Kind, Visitor},
        TokenKind,
    },
    print::PrintError,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Mode {
    Flat,
    Break,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct GroupId(usize);

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Cmd<'a> {
    Text(&'a str),
    TextIf(&'a str, Mode),
    Line,
    SoftLine,
    Space,
    Begin(Option<Mode>),
    End,
    Indent(usize),
    Dedent(usize),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct ListContext {
    len: u32,
    idx: u32,
    grp: GroupId,
}

pub struct Printer<'a, W: Write> {
    src: &'a str,
    out: W,
    width: usize,
    col: usize,
    indent: usize,

    pub queue: VecDeque<Cmd<'a>>,
    modes: Vec<Mode>,

    list_stack: Vec<ListContext>,

    pub error: Option<PrintError>,
}

impl<'a, W: Write> Printer<'a, W> {
    pub fn new(src: &'a str, out: W, width: usize) -> Self {
        Self {
            src,
            out,
            width,
            col: 0,
            indent: 0,
            queue: VecDeque::new(),
            modes: vec![Mode::Break],
            list_stack: Vec::new(),

            error: None,
        }
    }

    pub fn text(&mut self, txt: &'a str) {
        self.queue.push_back(Cmd::Text(txt));
    }

    pub fn text_if(&mut self, txt: &'a str, mode: Mode) {
        self.queue.push_back(Cmd::TextIf(txt, mode));
    }

    pub fn space(&mut self) {
        self.queue.push_back(Cmd::Space);
    }

    pub fn line(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        self.queue.push_back(Cmd::Line);
    }
    pub fn softline(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        self.queue.push_back(Cmd::SoftLine);
    }

    pub fn begin_group(&mut self, mode: Option<Mode>) -> GroupId {
        let id = GroupId(self.queue.len());
        self.queue.push_back(Cmd::Begin(mode));
        id
    }

    pub fn end_group(&mut self) {
        self.queue.push_back(Cmd::End);
    }

    pub fn indent(&mut self, n: usize) {
        self.queue.push_back(Cmd::Indent(n));
    }

    pub fn dedent(&mut self, n: usize) {
        self.queue.push_back(Cmd::Dedent(n));
    }

    fn fits(&self) -> bool {
        let mut col = self.col;
        let mut depth = 0;

        for cmd in &self.queue {
            match cmd {
                Cmd::Text(s) | Cmd::TextIf(s, Mode::Flat) => {
                    col += s.len();
                    if col > self.width {
                        return false;
                    }
                }
                Cmd::Line => return true,

                Cmd::Begin(_) => depth += 1,
                Cmd::End => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                }

                _ => {}
            }
        }

        true
    }

    pub fn flush(&mut self) -> fmt::Result {
        let mut block_space = false;
        let mut block_line = false;
        eprintln!("###### FLUSH ##");
        while let Some(cmd) = self.queue.pop_front() {
            eprintln!("- {cmd:?}");
            match cmd {
                Cmd::Text(s) => {
                    self.out.write_str(s)?;
                    self.col += s.len();
                    block_space = false;
                    block_line = false;
                }
                Cmd::TextIf(s, mode) => {
                    if *self.modes.last().unwrap() == mode {
                        self.out.write_str(s)?;
                        self.col += s.len();
                        block_space = false;
                        block_line = false;
                    }
                }
                Cmd::Space => {
                    if !block_space
                        && self
                            .queue
                            .front()
                            .is_some_and(|c| !matches!(c, Cmd::SoftLine | Cmd::Line))
                    {
                        self.out.write_char(' ')?;
                        self.col += 1;
                        block_space = true;
                        block_line = false;
                    }
                }

                Cmd::Line => {
                    self.out.write_char('\n')?;
                    for _ in 0..self.indent {
                        self.out.write_char(' ')?;
                    }
                    self.col = self.indent;
                    self.propagate_break();
                    block_space = true;
                    block_line = true;
                }

                Cmd::SoftLine => {
                    match self.modes.last().unwrap() {
                        Mode::Flat => {
                            if !block_space {
                                self.out.write_char(' ')?;
                                self.col += 1;
                                block_space = true;
                                block_line = false;
                            }
                        }
                        Mode::Break => {
                            if !block_line {
                                // eprintln!("  - not skipping");
                                self.out.write_char('\n')?;
                                for _ in 0..self.indent {
                                    self.out.write_char(' ')?;
                                }
                                self.col = self.indent;
                                self.propagate_break();
                                block_space = true;
                                block_line = true;
                            }
                        }
                    }
                }

                Cmd::Begin(mode) => {
                    self.modes.push(match mode {
                        Some(mode) => mode,
                        None => match self.fits() {
                            true => Mode::Flat,
                            false => Mode::Break,
                        },
                    });
                }
                Cmd::End => {
                    self.modes.pop();
                }

                Cmd::Indent(n) => {
                    self.indent += n;
                }
                Cmd::Dedent(n) => {
                    self.indent -= n;
                }

                _ => {}
            }
        }

        Ok(())
    }

    fn propagate_break(&mut self) {
        for group in self.modes.iter_mut().rev() {
            if *group == Mode::Flat {
                *group = Mode::Break;
            } else {
                break; // stop once we hit an already broken group
            }
        }
    }

    fn visit_token_inner(
        &mut self,
        token: &crate::parse::Token,
        context: &Cst,
    ) -> Result<(), PrintError> {
        let txt = self.src[token.span].trim();

        if txt.is_empty() {
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
                if let Some(Cmd::SoftLine) = self.queue.back() {
                    self.queue.pop_back();
                }
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

            _ => {
                self.text(txt);
            }
        }
        // self.flush()?;
        Ok(())
    }
}

impl<'a, W: fmt::Write> Visitor for Printer<'a, W> {
    fn enter_tree(&mut self, tree: &Cst) -> Visit {
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
                    if let Some(list) = self.list_stack.last_mut() {
                        let Cmd::Begin(mode) = self.queue.get_mut(list.grp.0).unwrap() else {
                            unreachable!("grp pointing at non begin cmd");
                        };
                        mode.replace(Mode::Break);
                    }
                }
                self.softline();
            }
            Kind::Entry => {
                self.line();
            }
            _ => {}
        }
        Visit::Continue
    }
    fn exit_tree(&mut self, tree: &Cst) -> Visit {
        match tree.kind {
            Kind::ListItemBlock | Kind::Block | Kind::TypeArgList => {
                self.list_stack.pop();
                self.end_group();
                // eprintln!("exit {} | stack: {}", tree.kind, self.list_stack.len());
                if let Some(list) = self.list_stack.last_mut() {
                    let Cmd::Begin(mode) = self.queue.get_mut(list.grp.0).unwrap() else {
                        unreachable!("grp pointing at non begin cmd");
                    };
                    mode.replace(Mode::Break);
                    self.softline();
                }
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
        Visit::Continue
    }
    fn visit_token(
        &mut self,
        token: &crate::parse::Token,
        context: &crate::parse::cst::Cst,
    ) -> Visit {
        match self.visit_token_inner(token, context) {
            Ok(_) => Visit::Continue,
            Err(e) => {
                self.error.replace(e);
                Visit::Stop
            }
        }
    }
}
