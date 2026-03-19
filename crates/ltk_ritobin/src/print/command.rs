#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Mode {
    Flat,
    Break,
}

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

impl Cmd<'_> {
    #[inline(always)]
    #[must_use]
    pub fn is_whitespace(&self) -> bool {
        matches!(self, Self::Space | Self::Line | Self::SoftLine)
    }
}
