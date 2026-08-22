use std::io;

use byteorder::{WriteBytesExt as _, LE};
use ltk_hash::WriteBytesExt as _;
use ltk_io_ext::{measure, window_at, WriterExt as _};

use crate::{
    data_override::{read::OVERRIDE_VERSION, BinOverride},
    traits::WriterExt as _,
    tree::write::WRITE_VERSION,
    BinKind,
};

impl<M: Clone> BinOverride<M> {
    /// Writes this patch to a writer.
    ///
    /// The output always uses `PTCH` version 1 around `PROP` version 3 with no dependencies,
    /// which is the only shape the client loads.
    ///
    /// # Arguments
    ///
    /// * `writer` - A writer that implements [`io::Write`] and [`io::Seek`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io::Cursor;
    /// use ltk_meta::BinOverride;
    ///
    /// let patch_bin = BinOverride::default();
    /// let mut buffer = Cursor::new(Vec::new());
    /// patch_bin.to_writer(&mut buffer)?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn to_writer<W: io::Write + io::Seek + ?Sized>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&BinKind::Override.magic())?;
        writer.write_u32::<LE>(OVERRIDE_VERSION)?;

        writer.write_u32::<LE>(self.deleted.len() as _)?;
        for &object_hash in &self.deleted {
            writer.write_bin_hash::<LE>(object_hash)?;
        }

        writer.write_all(&BinKind::Prop.magic())?;
        writer.write_u32::<LE>(WRITE_VERSION)?;
        // A patch that declares dependencies cannot be loaded, so there are never any.
        writer.write_u32::<LE>(0)?;

        writer.write_u32::<LE>(self.objects.len() as _)?;
        for object in self.objects.values() {
            writer.write_bin_hash::<LE>(object.class_hash)?;
        }
        for object in self.objects.values() {
            object.to_writer(writer)?;
        }

        writer.write_u32::<LE>(self.patches.len() as _)?;
        for patch in &self.patches {
            writer.write_bin_hash::<LE>(patch.object_hash)?;

            let size_pos = writer.stream_position()?;
            writer.write_u32::<LE>(0)?;

            let (size, _) = measure(writer, |writer| {
                writer.write_property_kind(patch.kind())?;
                writer.write_len_prefixed_string::<LE, _>(patch.path.as_str())?;
                patch.value.to_writer(writer)
            })?;

            window_at(writer, size_pos, |writer| writer.write_u32::<LE>(size as _))?;
        }

        Ok(())
    }
}
