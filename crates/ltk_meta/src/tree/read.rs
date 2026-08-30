use std::io::{self, SeekFrom};

use crate::{stream::BinStream, Error};

use super::{Bin, BinObject};
use byteorder::{ReadBytesExt as _, LE};
use indexmap::IndexMap;
use ltk_hash::{BinHash, ReadBytesExt as _};

impl Bin {
    /// Reads a BinTree from a reader.
    ///
    /// Mounts the reader as a [`BinStream`] and drains it, so the eager tree and the streaming
    /// surface are the same parser and can never drift. The reader is buffered internally and
    /// is not left at a defined position afterwards.
    ///
    /// # Arguments
    ///
    /// * `reader` - A reader that implements io::Read and io::Seek.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnexpectedBinKind`] for a `PTCH` file - read those with
    /// [`BinOverride::from_reader`](crate::BinOverride::from_reader), or with
    /// [`BinFile::from_reader`](crate::BinFile::from_reader) when the kind is not known in
    /// advance. See [`BinStream::into_bin`] for what a malformed `PROP` file raises.
    pub fn from_reader<R: io::Read + std::io::Seek + ?Sized>(
        reader: &mut R,
    ) -> Result<Self, Error> {
        BinStream::mount(reader)?.into_bin()
    }
}

/// Reads a class table and the objects it describes.
///
/// Shared by [`Bin`] and [`BinOverride`](crate::BinOverride), which hold the same object table.
/// Returns the objects and whether they had to be read with the legacy property kind numbering,
/// which the caller needs to keep reading the rest of the file the same way.
pub(crate) fn read_objects<M: Default, R: io::Read + io::Seek + ?Sized>(
    reader: &mut R,
) -> Result<(IndexMap<BinHash, BinObject<M>>, bool), Error> {
    let count = reader.read_u32::<LE>()? as usize;
    let mut classes = Vec::with_capacity(count);
    for _ in 0..count {
        classes.push(reader.read_bin_hash::<LE>()?);
    }

    let start = reader.stream_position()?;
    let mut objects = IndexMap::with_capacity(count);
    match try_read_objects(reader, &classes, &mut objects, false) {
        Ok(()) => Ok((objects, false)),
        Err(Error::InvalidPropertyTypePrimitive(kind)) => {
            log::warn!("Invalid prop type {kind}. Trying reading objects as legacy.");
            reader.seek(SeekFrom::Start(start))?;
            try_read_objects(reader, &classes, &mut objects, true)?;
            Ok((objects, true))
        }
        Err(e) => Err(e),
    }
}

fn try_read_objects<M: Default, R: io::Read + io::Seek + ?Sized>(
    reader: &mut R,
    classes: &[BinHash],
    objects: &mut IndexMap<BinHash, BinObject<M>>,
    legacy: bool,
) -> Result<(), Error> {
    objects.clear();
    for &class_hash in classes {
        let object = BinObject::from_reader(reader, class_hash, legacy)?;
        objects.insert(object.path_hash, object);
    }
    Ok(())
}
