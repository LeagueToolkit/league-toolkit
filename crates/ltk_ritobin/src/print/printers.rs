use std::fmt::{self, Write};

use ltk_meta::{
    property::{
        values::{Embedded, Struct, UnorderedContainer},
        PropertyValueEnum,
    },
    Bin, BinObject, BinProperty,
};

use crate::{
    cst::{self, Cst},
    hashes::HashProvider,
    print::{PrintConfig, PrintError},
    types::kind_to_type_name,
};

pub struct CstPrinter<'a, W: Write, H: HashProvider> {
    visitor: super::visitor::CstVisitor<'a, W, H>,
}

impl<'a, W: Write, H: HashProvider> CstPrinter<'a, W, H> {
    pub fn new(src: &'a str, out: W, config: PrintConfig<H>) -> Self {
        Self {
            visitor: super::visitor::CstVisitor::new(src, out, config),
        }
    }

    pub fn print(mut self, cst: &Cst) -> Result<usize, PrintError> {
        cst.walk(&mut self.visitor);
        self.visitor.flush()?;
        if let Some(e) = self.visitor.error {
            return Err(e);
        }
        eprintln!("max q size: {}", self.visitor.queue_size_max);
        Ok(self.visitor.printed_bytes)
    }
}

/// Text writer for ritobin format with hash provider support.
pub struct BinPrinter<H: HashProvider = ()> {
    buffer: String,
    config: PrintConfig<H>,
}

impl BinPrinter<()> {
    /// Create a new text writer without hash lookup (all hashes written as hex).
    pub fn new() -> Self {
        Self::default()
    }
}

impl<H: HashProvider> BinPrinter<H> {
    /// Create a new printer with custom configuration and hash provider.
    pub fn with_config<H2: HashProvider>(self, config: PrintConfig<H2>) -> BinPrinter<H2> {
        BinPrinter {
            buffer: self.buffer,
            config,
        }
    }

    pub fn print<W: fmt::Write + ?Sized>(
        &mut self,
        tree: &Bin,
        writer: &mut W,
    ) -> Result<usize, PrintError> {
        let mut builder = cst::builder::Builder::new();
        let cst = builder.build(tree);
        let buf = builder.into_text_buffer();
        CstPrinter::new(&buf, writer, Default::default()).print(&cst)
    }

    pub fn print_to_string(&mut self, tree: &Bin) -> Result<String, PrintError> {
        let mut str = String::new();
        self.print(tree, &mut str)?;
        Ok(str)
    }
}
impl Default for BinPrinter<()> {
    fn default() -> Self {
        Self {
            buffer: String::new(),
            config: PrintConfig::default(),
        }
    }
}
