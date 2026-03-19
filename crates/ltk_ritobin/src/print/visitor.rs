use std::{
    collections::VecDeque,
    fmt::{self, Write},
};

use crate::{
    cst::{visitor::Visit, Cst, Kind, Visitor},
    parse::TokenKind,
    print::{
        command::{Cmd, Mode},
        PrintConfig, PrintError,
    },
    HashProvider,
};

mod r#impl;
pub mod state;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GroupId(usize);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct ListContext {
    len: u32,
    idx: u32,
    grp: GroupId,
}

const MAX_QUEUE: usize = 4096;

pub struct CstVisitor<'a, W: Write, H: HashProvider> {
    config: PrintConfig<H>,

    src: &'a str,
    out: W,
    col: usize,
    indent: usize,

    pub printed_bytes: usize,
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

impl<'a, W: Write, H: HashProvider> CstVisitor<'a, W, H> {
    pub fn new(src: &'a str, out: W, config: PrintConfig<H>) -> Self {
        Self {
            src,
            out,
            config,
            col: 0,
            indent: 0,
            printed: 0,
            printed_bytes: 0,
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
        if self.queue.back().is_some_and(|c| c.is_whitespace()) {
            // eprintln!("# skipping Space! ({:?})", self.queue.back());
            return;
        }
        self.push(Cmd::Space);
    }

    pub fn line(&mut self) {
        if self.queue.is_empty() && self.printed == 0 {
            return;
        }
        if self.queue.back().is_some_and(|c| c.is_whitespace()) {
            // eprintln!("# replacing ({:?}) w/ Line", self.queue.back());
            self.queue.pop_back();
        }
        self.push(Cmd::Line);
    }
    pub fn softline(&mut self) {
        if self.queue.is_empty() && self.printed == 0 {
            return;
        }
        if self.queue.back().is_some_and(|c| c.is_whitespace()) {
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
            if self.col + size > self.config.line_width {
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
                    if col > self.config.line_width {
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
                self.printed_bytes += s.len();
                self.col += s.len();
                self.block_space = false;
                self.block_line = false;
            }
            Cmd::TextIf(s, mode) => {
                if *self.modes.last().unwrap() == mode {
                    self.out.write_str(s)?;
                    self.printed_bytes += s.len();
                    self.col += s.len();
                    self.block_space = false;
                    self.block_line = false;
                }
            }
            Cmd::Space => {
                if !self.block_space {
                    self.out.write_char(' ')?;
                    self.printed_bytes += 1;
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
                self.printed_bytes += self.indent + 1;
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
                        self.printed_bytes += 1;
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
                        self.printed_bytes += self.indent + 1;
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
}
