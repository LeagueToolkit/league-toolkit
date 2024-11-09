use std::{
    borrow::Cow,
    io::{BufReader, Read},
};

use byteorder::{ReadBytesExt as _, LE};
use io_ext::ReaderExt as _;

use crate::error::ModpkgError;

#[derive(Debug, PartialEq, PartialOrd)]
pub struct ModpkgChunk {
    path: Cow<'static, str>,
    path_hash: u64,
    compressed_size: usize,
    uncompressed_size: usize,
    data_offset: usize,
    checksum: u64,
}

impl ModpkgChunk {
    pub fn read(reader: &mut BufReader<impl Read>) -> Result<Self, ModpkgError> {
        let path = reader.read_len_prefixed_string::<LE>()?;
        let path_hash = reader.read_u64::<LE>()?;
        let compressed_size = reader.read_u64::<LE>()?;
        let uncompressed_size = reader.read_u64::<LE>()?;
        let data_offset = reader.read_u64::<LE>()?;
        let checksum = reader.read_u64::<LE>()?;

        Ok(Self {
            path: Cow::from(path),
            path_hash,
            compressed_size: compressed_size as usize,
            uncompressed_size: uncompressed_size as usize,
            data_offset: data_offset as usize,
            checksum,
        })
    }

    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn path_hash(&self) -> u64 {
        self.path_hash
    }
    pub fn compressed_size(&self) -> usize {
        self.compressed_size
    }
    pub fn uncompressed_size(&self) -> usize {
        self.uncompressed_size
    }
    pub fn data_offset(&self) -> usize {
        self.data_offset
    }
    pub fn checksum(&self) -> u64 {
        self.checksum
    }
}
