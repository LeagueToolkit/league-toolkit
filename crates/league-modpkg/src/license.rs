use std::io::{self, BufReader};

use byteorder::{ReadBytesExt as _, LE};
use io_ext::ReaderExt as _;

use crate::error::ModpkgError;

#[derive(Debug, PartialEq)]
pub enum ModpkgLicense {
    None,
    Spdx { spdx_id: String },
    Custom { name: String, url: String },
}

impl ModpkgLicense {
    pub fn read(reader: &mut BufReader<impl io::Read>) -> Result<Self, ModpkgError> {
        let license_type = reader.read_u8()?;

        Ok(match license_type {
            0 => Self::None,
            1 => {
                let spdx_id = reader.read_len_prefixed_string::<LE>()?;
                Self::Spdx { spdx_id }
            }
            2 => {
                let name = reader.read_len_prefixed_string::<LE>()?;
                let url = reader.read_len_prefixed_string::<LE>()?;
                Self::Custom { name, url }
            }
            _ => return Err(ModpkgError::InvalidLicenseType(license_type)),
        })
    }

    pub fn write(&self, writer: &mut impl io::Write) -> Result<(), ModpkgError> {
        unimplemented!("TODO: modpkg writing");
    }
}
