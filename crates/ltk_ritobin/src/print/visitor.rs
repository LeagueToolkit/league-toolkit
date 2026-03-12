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
pub struct GroupId(usize);

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

const MAX_QUEUE: usize = 4096;

pub struct Printer<'a, W: Write> {
    src: &'a str,
    out: W,
    width: usize,
    col: usize,
    indent: usize,

    printed: usize,
    pub queue: VecDeque<Cmd<'a>>,
    pub queue_size_max: usize,
    modes: Vec<Mode>,

    /// group start indices
    group_stack: Vec<usize>,
    list_stack: Vec<ListContext>,
    /// running inline size for each group
    size_stack: Vec<usize>,

    pub error: Option<PrintError>,

    block_space: bool,
    block_line: bool,
}

impl<'a, W: Write> Printer<'a, W> {
    pub fn new(src: &'a str, out: W, width: usize) -> Self {
        Self {
            src,
            out,
            width,
            col: 0,
            indent: 0,
            printed: 0,
            queue: VecDeque::new(),
            queue_size_max: 0,
            modes: vec![Mode::Break],

            group_stack: Vec::new(),
            list_stack: Vec::new(),
            size_stack: Vec::new(),

            error: None,

            block_space: false,
            block_line: false,
        }
    }

    fn push(&mut self, cmd: Cmd<'a>) {
        // eprintln!("+ {cmd:?}");
        self.queue.push_back(cmd);
        self.queue_size_max = self.queue_size_max.max(self.queue.len());
        if self.queue.len() > MAX_QUEUE {
            eprintln!("[!!] hit hard queue limit - force breaking to save memory");
            self.break_first_group();
        }
    }

    pub fn text(&mut self, txt: &'a str) {
        for size in &mut self.size_stack {
            *size += txt.len();
        }
        self.check_running_size();
        self.push(Cmd::Text(txt));
    }

    pub fn text_if(&mut self, txt: &'a str, mode: Mode) {
        self.push(Cmd::TextIf(txt, mode));
    }

    pub fn space(&mut self) {
        if matches!(
            self.queue.back(),
            Some(Cmd::Space) | Some(Cmd::Line) | Some(Cmd::SoftLine)
        ) {
            // eprintln!("# skipping Space! ({:?})", self.queue.back());
            return;
        }
        self.push(Cmd::Space);
    }

    pub fn line(&mut self) {
        if self.queue.is_empty() && self.printed == 0 {
            return;
        }
        if matches!(
            self.queue.back(),
            Some(Cmd::Space) | Some(Cmd::Line) | Some(Cmd::SoftLine)
        ) {
            // eprintln!("# replacing ({:?}) w/ Line", self.queue.back());
            self.queue.pop_back();
        }
        self.push(Cmd::Line);
    }
    pub fn softline(&mut self) {
        if self.queue.is_empty() && self.printed == 0 {
            return;
        }
        if matches!(
            self.queue.back(),
            Some(Cmd::Space) | Some(Cmd::Line) | Some(Cmd::SoftLine)
        ) {
            // eprintln!("# replacing ({:?}) w/ SoftLine", self.queue.back());
            self.queue.pop_back();
        }
        self.push(Cmd::SoftLine);
    }

    pub fn begin_group(&mut self, mode: Option<Mode>) -> GroupId {
        let idx = self.queue.len() + self.printed;

        self.push(Cmd::Begin(mode));

        self.group_stack.push(idx);
        self.size_stack.push(0);

        GroupId(idx)
    }

    pub fn end_group(&mut self) {
        self.push(Cmd::End);

        self.group_stack.pop();
        self.size_stack.pop();

        self.print_safe().unwrap();
    }

    pub fn indent(&mut self, n: usize) {
        self.push(Cmd::Indent(n));
    }

    pub fn dedent(&mut self, n: usize) {
        self.push(Cmd::Dedent(n));
    }

    /// break the first/oldest group if running size is too big (bottom of the stack)
    pub fn break_first_group(&mut self) {
        if let Some(&idx) = self.group_stack.first() {
            // eprintln!("[printer] breaking first group");
            self.force_group(GroupId(idx), Mode::Break);

            self.group_stack.remove(0);
            self.size_stack.remove(0);
        }
        self.print_safe().unwrap();
    }

    /// break the first/oldest group if running size is too big (bottom of the stack)
    pub fn check_running_size(&mut self) {
        if let Some(size) = self.size_stack.last() {
            if self.col + size > self.width {
                self.break_first_group();
            }
        }
    }

    fn fits(&self) -> bool {
        let mut col = self.col;
        let mut depth = 0;

        for (i, cmd) in self.queue.iter().enumerate() {
            if i > 512 {
                panic!("fits too long");
            }
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

    pub fn force_group(&mut self, group: GroupId, mode: Mode) {
        if group.0 < self.printed {
            // eprintln!("[!!] trying to mutate already printed group! {group:?}");
            return;
            // panic!("trying to mutate already printed group!");
        }
        let cmd = self.queue.get_mut(group.0 - self.printed).unwrap();
        let Cmd::Begin(grp_mode) = cmd else {
            unreachable!("grp pointing at non begin cmd {cmd:?}");
        };
        grp_mode.replace(mode);
    }

    fn print(&mut self, cmd: Cmd) -> fmt::Result {
        match cmd {
            Cmd::Text(s) => {
                self.out.write_str(s)?;
                self.col += s.len();
                self.block_space = false;
                self.block_line = false;
            }
            Cmd::TextIf(s, mode) => {
                if *self.modes.last().unwrap() == mode {
                    self.out.write_str(s)?;
                    self.col += s.len();
                    self.block_space = false;
                    self.block_line = false;
                }
            }
            Cmd::Space => {
                if !self.block_space {
                    self.out.write_char(' ')?;
                    self.col += 1;
                    self.block_space = true;
                    self.block_line = false;
                }
            }

            Cmd::Line => {
                self.out.write_char('\n')?;
                for _ in 0..self.indent {
                    self.out.write_char(' ')?;
                }
                self.col = self.indent;
                self.propagate_break();
                self.block_space = true;
                self.block_line = true;
            }

            Cmd::SoftLine => match self.modes.last().unwrap() {
                Mode::Flat => {
                    if !self.block_space {
                        // eprintln!("  - not skipping -> space!");
                        self.out.write_char(' ')?;
                        self.col += 1;
                        self.block_space = true;
                        self.block_line = false;
                    }
                }
                Mode::Break => {
                    if !self.block_line {
                        // eprintln!("  - not skipping -> line!");
                        self.out.write_char('\n')?;
                        for _ in 0..self.indent {
                            self.out.write_char(' ')?;
                        }
                        self.col = self.indent;
                        self.propagate_break();
                        self.block_space = true;
                        self.block_line = true;
                    }
                }
            },

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
                self.indent = self.indent.saturating_sub(n);
            }

            _ => {}
        }
        Ok(())
    }

    pub fn flush(&mut self) -> fmt::Result {
        // eprintln!("###### FLUSH ##");
        while let Some(cmd) = self.queue.pop_front() {
            self.printed += 1;
            self.print(cmd)?;
            // eprintln!("- {cmd:?}");
        }

        Ok(())
    }

    pub fn print_safe(&mut self) -> fmt::Result {
        let limit = self
            .group_stack
            .first()
            .copied()
            .unwrap_or(self.queue.len() + self.printed);

        if limit > self.printed {
            let count = limit - self.printed;
            // eprintln!("[printer] printing {count}...");
        }

        while self.printed < limit {
            let cmd = self.queue.pop_front().unwrap();
            // eprintln!("- {cmd:?}");
            self.printed += 1;
            self.print(cmd)?;
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
        self.print_safe()?;
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
                    if let Some(list) = self.list_stack.last() {
                        self.force_group(list.grp, Mode::Break);
                    }
                }
                self.softline();
            }
            Kind::Entry => {
                self.line();
                // self.flush().unwrap();
            }
            _ => {}
        }
        Visit::Continue
    }
    fn exit_tree(&mut self, tree: &Cst) -> Visit {
        match tree.kind {
            Kind::TypeArgList => {
                self.list_stack.pop();
                self.end_group();
            }
            Kind::ListItemBlock | Kind::Block => {
                self.list_stack.pop();
                // eprintln!("exit {} | stack: {}", tree.kind, self.list_stack.len());
                if let Some(list) = self.list_stack.last() {
                    self.force_group(list.grp, Mode::Break);
                    self.softline();
                }
                self.end_group();
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
