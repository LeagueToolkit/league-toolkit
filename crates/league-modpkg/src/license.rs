use std::io::{BufReader, Read, Write};

use byteorder::{ReadBytesExt as _, WriteBytesExt as _, LE};
use io_ext::{ReaderExt as _, WriterExt as _};

use crate::error::ModpkgError;

#[derive(Debug, PartialEq, Default)]
pub enum ModpkgLicense {
    #[default]
    None,
    Spdx {
        spdx_id: String,
    },
    Custom {
        name: String,
        url: String,
    },
}

impl ModpkgLicense {
    pub fn read(reader: &mut BufReader<impl Read>) -> Result<Self, ModpkgError> {
        let license_type = reader.read_u8()?;
        match license_type {
            0 => Ok(Self::None),
            1 => Ok(Self::Spdx {
                spdx_id: reader.read_len_prefixed_string::<LE>()?,
            }),
            2 => Ok(Self::Custom {
                name: reader.read_len_prefixed_string::<LE>()?,
                url: reader.read_len_prefixed_string::<LE>()?,
            }),
            _ => Err(ModpkgError::InvalidLicenseType(license_type)),
        }
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), ModpkgError> {
        writer.write_u8(match self {
            Self::None => 0,
            Self::Spdx { .. } => 1,
            Self::Custom { .. } => 2,
        })?;

        match self {
            Self::Spdx { spdx_id } => {
                writer.write_len_prefixed_string_better::<LE>(spdx_id)?;

                Ok(())
            }
            Self::Custom { name, url } => {
                writer.write_len_prefixed_string_better::<LE>(name)?;
                writer.write_len_prefixed_string_better::<LE>(url)?;

                Ok(())
            }
            Self::None => Ok(()),
        }
    }

    pub fn size(&self) -> usize {
        match self {
            Self::None => 1,
            Self::Spdx { spdx_id } => 1 + 4 + spdx_id.len(),
            Self::Custom { name, url } => 1 + 4 + 4 + name.len() + url.len(),
        }
    }
}
