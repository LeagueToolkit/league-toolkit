use crate::HashProvider;

#[derive(Debug, Clone, Copy)]
pub struct WrapConfig {
    /// Maximum line width - will try to break blocks if the line exceeds this number.
    pub line_width: usize,

    /// Whether to allow the printing of structs/classes in one line
    /// (subject to [`Self::line_width`] wrapping)
    pub inline_structs: bool,

    /// Whether to allow the printing of lists in one line
    /// (subject to [`Self::line_width`] wrapping)
    pub inline_lists: bool,
}

impl WrapConfig {
    pub fn line_width(mut self, line_width: usize) -> Self {
        self.line_width = line_width;
        self
    }

    pub fn inline_structs(mut self, inline_structs: bool) -> Self {
        self.inline_structs = inline_structs;
        self
    }

    pub fn inline_lists(mut self, inline_lists: bool) -> Self {
        self.inline_lists = inline_lists;
        self
    }
}

impl Default for WrapConfig {
    fn default() -> Self {
        Self {
            line_width: 120,
            inline_structs: false,
            inline_lists: true,
        }
    }
}

/// Configuration for the ritobin printer.
#[derive(Debug, Clone)]
pub struct PrintConfig<Hashes: HashProvider> {
    /// Number of spaces per indent level.
    pub indent_size: usize,

    /// Config relating to how/when to wrap
    pub wrap: WrapConfig,

    pub hashes: Hashes,
}

impl Default for PrintConfig<()> {
    fn default() -> Self {
        Self {
            indent_size: 4,
            wrap: Default::default(),
            hashes: (),
        }
    }
}

impl<H: HashProvider> PrintConfig<H> {
    pub fn wrap(mut self, wrap: WrapConfig) -> Self {
        self.wrap = wrap;
        self
    }
    pub fn indent_size(mut self, indent_size: usize) -> Self {
        self.indent_size = indent_size;
        self
    }

    pub fn with_hashes<H2: HashProvider>(self, hashes: H2) -> PrintConfig<H2> {
        PrintConfig {
            indent_size: self.indent_size,
            wrap: self.wrap,
            hashes,
        }
    }
}
