use std::io;

use crate::header::PropHeader;

use super::Bin;
use byteorder::{WriteBytesExt as _, LE};
use ltk_hash::WriteBytesExt as _;
use ltk_primitives::StringWriteError;

#[derive(Debug, thiserror::Error)]
pub enum BinWriteError {
    #[error(transparent)]
    StringWrite(#[from] StringWriteError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl Bin {
    /// Write this bin to a writer.
    ///
    /// The output will always use the latest format we support ([`PropHeader::LATEST_VERSION`]), regardless of the
    /// `version` field on this struct. This ensures maximum compatibility
    /// and feature support.
    ///
    /// # Arguments
    ///
    /// * `writer` - A writer that implements io::Write and io::Seek.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io::Cursor;
    /// use ltk_meta::Bin;
    ///
    /// let tree = Bin::default();
    /// let mut buffer = Cursor::new(Vec::new());
    /// tree.to_writer(&mut buffer)?;
    /// # Ok::<(), ltk_meta::BinWriteError>(())
    /// ```
    pub fn to_writer<W: io::Write + io::Seek + ?Sized>(
        &self,
        writer: &mut W,
    ) -> Result<(), BinWriteError> {
        writer.write_u32::<LE>(PropHeader::MAGIC)?;

        writer.write_u32::<LE>(self.version)?;
        writer.write_u32::<LE>(self.dependencies.len().try_into().unwrap())?;
        for dep in &self.dependencies {
            dep.to_writer(writer)?;
        }

        writer.write_u32::<LE>(self.objects.len().try_into().unwrap())?;
        for obj in self.objects.values() {
            writer.write_bin_hash::<LE>(obj.class_hash)?;
        }
        for obj in self.objects.values() {
            obj.to_writer(writer)?;
        }

        if self.is_override {
            writer.write_u32::<LE>(self.data_overrides.len() as _)?;
            // TODO: impl data overrides
            //for o in &self.data_overrides {
            //}
        }

        Ok(())
    }
}
