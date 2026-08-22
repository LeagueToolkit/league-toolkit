use std::io;

use byteorder::{ReadBytesExt as _, LE};
use ltk_hash::{BinHash, ReadBytesExt as _};
use ltk_io_ext::{measure, ReaderExt as _};

use crate::{
    data_override::{BinOverride, PropertyPatch},
    path::PropertyPath,
    traits::ReaderExt as _,
    tree::read::read_objects,
    BinKind, Error, PropertyValueEnum,
};

/// The only `PTCH` container version the client accepts.
pub(super) const OVERRIDE_VERSION: u32 = 1;

impl BinOverride {
    /// Reads a `PTCH` bin from a reader.
    ///
    /// # Arguments
    ///
    /// * `reader` - A reader that implements [`io::Read`] and [`io::Seek`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnexpectedBinKind`] for a `PROP` file, and
    /// [`Error::OverrideDependencies`] for a patch that declares dependencies, which no client
    /// can load.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use ltk_meta::BinOverride;
    ///
    /// let mut file = File::open("uiflipped.bin")?;
    /// let patch_bin = BinOverride::from_reader(&mut file)?;
    ///
    /// for patch in &patch_bin.patches {
    ///     println!("{:08x} {}", patch.object_hash, patch.path);
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_reader<R: io::Read + io::Seek + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        match BinKind::from_magic_u32(reader.read_u32::<LE>()?) {
            Some(BinKind::Override) => {}
            Some(found) => {
                return Err(Error::UnexpectedBinKind {
                    expected: BinKind::Override,
                    found,
                })
            }
            None => return Err(Error::InvalidFileSignature),
        }

        let version = reader.read_u32::<LE>()?;
        if version != OVERRIDE_VERSION {
            return Err(Error::InvalidOverrideVersion(version));
        }

        let delete_count = reader.read_u32::<LE>()? as usize;
        let mut deleted = Vec::with_capacity(delete_count);
        for _ in 0..delete_count {
            deleted.push(reader.read_bin_hash::<LE>()?);
        }

        // The patch's own contents are a plain PROP section, minus the dependencies.
        if BinKind::from_magic_u32(reader.read_u32::<LE>()?) != Some(BinKind::Prop) {
            log::error!("Expected a PROP section inside the PTCH container");
            return Err(Error::InvalidFileSignature);
        }

        let version = reader.read_u32::<LE>()?;
        if !matches!(version, 1..=3) {
            return Err(Error::InvalidFileVersion(version));
        }

        if version >= 2 {
            // The client does not support dependencies, so we reject any patch that declares them.
            let dependency_count = reader.read_u32::<LE>()?;
            if dependency_count != 0 {
                return Err(Error::OverrideDependencies(dependency_count));
            }
        }

        let (objects, _legacy) = read_objects(reader)?;
        let patches = match version {
            3.. => read_patches(reader)?,
            _ => Vec::new(),
        };

        Ok(Self {
            deleted,
            objects,
            patches,
        })
    }
}

fn read_patches<R: io::Read + io::Seek + ?Sized>(
    reader: &mut R,
) -> Result<Vec<PropertyPatch>, Error> {
    let count = reader.read_u32::<LE>()? as usize;
    let mut patches = Vec::with_capacity(count);
    for index in 0..count {
        patches.push(read_patch(reader, index)?);
    }
    Ok(patches)
}

fn read_patch<R: io::Read + io::Seek + ?Sized>(
    reader: &mut R,
    index: usize,
) -> Result<PropertyPatch, Error> {
    let object_hash = reader.read_bin_hash::<LE>()?;
    let size = reader.read_u32::<LE>()?;

    let (real_size, patch) = measure(reader, |reader| {
        let kind = reader.read_property_kind(false)?;
        let path = read_path(reader, index, object_hash)?;
        let value = PropertyValueEnum::from_reader(reader, kind, false)?;

        Ok::<_, Error>(PropertyPatch {
            object_hash,
            path,
            value,
        })
    })?;

    if size as u64 != real_size {
        return Err(Error::InvalidSize(size as _, real_size));
    }
    Ok(patch)
}

fn read_path<R: io::Read + ?Sized>(
    reader: &mut R,
    index: usize,
    object_hash: BinHash,
) -> Result<PropertyPath, Error> {
    let path = reader.read_sized_string_u16::<LE>()?;
    PropertyPath::new(path).map_err(|source| Error::InvalidPropertyPath {
        index,
        object_hash,
        source,
    })
}
