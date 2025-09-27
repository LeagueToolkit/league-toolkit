use std::io;

use super::BinTree;
use byteorder::{WriteBytesExt, LE};
use ltk_io_ext::WriterExt;

impl BinTree {
    /// Writes a BinTree to a writer.
    ///
    /// # Arguments
    ///
    /// * `writer` - A writer that implements io::Write and io::Seek.
    /// * `legacy` - Whether to write in legacy format.
    pub fn to_writer<W: io::Write + io::Seek + ?Sized>(
        &self,
        writer: &mut W,
        legacy: bool,
    ) -> io::Result<()> {
        match self.is_override {
            true => todo!("implement is_override BinTree write"),
            false => {
                writer.write_u32::<LE>(Self::PROP)?;
            }
        }

        writer.write_u32::<LE>(self.version)?;

        if !self.dependencies.is_empty() && self.version < 2 {
            // FIXME: move this assertion to object creation
            panic!(
                "cannot write BinTree with dependencies @ version {}",
                self.version
            );
        }

        if self.version >= 2 {
            writer.write_u32::<LE>(self.dependencies.len() as _)?;
            for dep in &self.dependencies {
                writer.write_len_prefixed_string::<LE, _>(dep)?;
            }
        }

        writer.write_u32::<LE>(self.objects.len() as _)?;
        for obj in self.objects.values() {
            writer.write_u32::<LE>(obj.class_hash)?;
        }
        for obj in self.objects.values() {
            obj.to_writer(writer, legacy)?;
        }

        if self.is_override {
            if self.version < 3 {
                panic!("cannot write data overrides @ version {}", self.version);
            }
            writer.write_u32::<LE>(self.data_overrides.len() as _)?;
            // TODO: impl data overrides
            //for o in &self.data_overrides {
            //}
        }

        Ok(())
    }
}
