use crate::HashProvider;

/// Configuration for the ritobin printer.
#[derive(Debug, Clone)]
pub struct PrintConfig<Hashes: HashProvider> {
    /// Number of spaces per indent level.
    pub indent_size: usize,
    /// Maximum line width
    pub line_width: usize,

    pub hashes: Hashes,
}

impl Default for PrintConfig<()> {
    fn default() -> Self {
        Self {
            indent_size: 4,
            line_width: 120,
            hashes: (),
        }
    }
}

impl<H: HashProvider> PrintConfig<H> {
    pub fn with_hashes<H2: HashProvider>(self, hashes: H2) -> PrintConfig<H2> {
        PrintConfig {
            indent_size: self.indent_size,
            line_width: self.line_width,
            hashes,
        }
    }
}
