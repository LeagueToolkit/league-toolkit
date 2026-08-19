use std::io::{Read, Write};

use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use ltk_primitives::PrefixString;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropHeader {
    pub version: u32,
    /// List of other property bins this file depends on.
    ///
    /// Property bins can depend on other property bins in a similar fashion
    /// to importing code libraries.
    pub dependencies: Option<Vec<PrefixString<u16>>>,
    pub object_count: u32,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchHeader {
    pub version: u32,
    pub override_count: u32,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Header {
    Prop(PropHeader),
    Patch(PatchHeader, PropHeader),
}

impl From<PropHeader> for Header {
    fn from(value: PropHeader) -> Self {
        Self::Prop(value)
    }
}
impl From<(PatchHeader, PropHeader)> for Header {
    fn from(value: (PatchHeader, PropHeader)) -> Self {
        Self::Patch(value.0, value.1)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ReadHeaderError {
    #[error("Invalid file signature")]
    InvalidFileSignature,
    #[error("Invalid file version '{0}'")]
    InvalidFileVersion(u32),

    #[error(transparent)]
    ReaderError(#[from] ltk_io_ext::ReaderError),
    #[error(transparent)]
    StringReadError(#[from] ltk_primitives::StringReadError),

    #[error("IO Error - {0}")]
    IOError(#[from] std::io::Error),
}
#[derive(Debug, thiserror::Error)]
pub enum WriteHeaderError {
    #[error(transparent)]
    StringWriteError(#[from] ltk_primitives::StringWriteError),

    #[error("IO Error - {0}")]
    IOError(#[from] std::io::Error),
}
impl Header {
    pub fn from_reader(reader: &mut (impl Read + ?Sized)) -> Result<Self, ReadHeaderError> {
        let magic = reader.read_u32::<LE>()?;
        match magic {
            PropHeader::MAGIC => PropHeader::from_reader(reader).map(Self::Prop),
            PatchHeader::MAGIC => Ok(Self::Patch(
                PatchHeader::from_reader(reader)?,
                PropHeader::from_reader(reader)?,
            )),
            _ => Err(ReadHeaderError::InvalidFileSignature),
        }
    }

    pub fn to_writer(&self, writer: &mut (impl Write + ?Sized)) -> Result<(), WriteHeaderError> {
        match self {
            Header::Prop(prop_header) => {
                writer.write_u32::<LE>(PropHeader::MAGIC)?;
                prop_header.to_writer(writer)?;
            }
            Header::Patch(patch_header, prop_header) => {
                writer.write_u32::<LE>(PatchHeader::MAGIC)?;
                patch_header.to_writer(writer)?;
                writer.write_u32::<LE>(PropHeader::MAGIC)?;
                prop_header.to_writer(writer)?;
            }
        }
        Ok(())
    }

    pub fn prop_header(&self) -> &PropHeader {
        match &self {
            Header::Prop(prop) => prop,
            Header::Patch(_, prop) => prop,
        }
    }
    pub fn into_parts(self) -> (PropHeader, Option<PatchHeader>) {
        match self {
            Header::Prop(prop) => (prop, None),
            Header::Patch(patch, prop) => (prop, Some(patch)),
        }
    }
}

impl PropHeader {
    pub const LATEST_VERSION: u32 = 3;
    pub const MAGIC: u32 = u32::from_le_bytes(*b"PROP");
    pub fn from_reader(reader: &mut (impl Read + ?Sized)) -> Result<Self, ReadHeaderError> {
        use ReadHeaderError::*;

        let version = reader.read_u32::<LE>()?;
        if !matches!(version, 1..=3) {
            // TODO (alan): distinguish override/non-override version in err?
            return Err(InvalidFileVersion(version));
        }

        let dependencies = match version {
            2.. => {
                let dep_count = reader.read_u32::<LE>()?;
                let mut dependencies = Vec::with_capacity(dep_count as _);
                for _ in 0..dep_count {
                    dependencies.push(PrefixString::from_reader(reader)?);
                }
                Some(dependencies)
            }
            _ => None,
        };

        let object_count = reader.read_u32::<LE>()?;

        Ok(Self {
            version,
            dependencies,
            object_count,
        })
    }

    pub fn to_writer(&self, writer: &mut (impl Write + ?Sized)) -> Result<(), WriteHeaderError> {
        writer.write_u32::<LE>(self.version)?;
        writer.write_u32::<LE>(
            self.dependencies
                .as_ref()
                .map(|d| d.len().try_into().unwrap())
                .unwrap_or_default(),
        )?;

        if let Some(deps) = self.dependencies.as_ref() {
            for dep in deps {
                dep.to_writer(writer)?;
            }
        }

        writer.write_u32::<LE>(self.object_count)?;

        Ok(())
    }
}

impl PatchHeader {
    pub const MAGIC: u32 = u32::from_le_bytes(*b"PTCH");
    pub fn from_reader(reader: &mut (impl Read + ?Sized)) -> Result<Self, ReadHeaderError> {
        use ReadHeaderError::*;

        let version = reader.read_u32::<LE>()?;
        if version != 1 {
            return Err(InvalidFileVersion(version));
        }

        let override_count = reader.read_u32::<LE>()?;

        let magic = reader.read_u32::<LE>()?;
        if magic != PropHeader::MAGIC {
            // TODO (alan): repr this in the error
            log::error!(
                "Expected PROP ({:#x}) section after PTCH ({:#x}), got '{:#x}'",
                PropHeader::MAGIC,
                PatchHeader::MAGIC,
                magic
            );
            return Err(InvalidFileSignature);
        }

        Ok(Self {
            version,
            override_count,
        })
    }

    pub fn to_writer(&self, _writer: &mut (impl Write + ?Sized)) -> Result<(), WriteHeaderError> {
        todo!("TODO: implement PTCH header write");
    }
}
