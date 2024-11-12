use crate::error::ModpkgError;
use byteorder::{ReadBytesExt as _, LE};
use std::io::{BufReader, Read};

#[derive(Debug, PartialEq, PartialOrd)]
pub struct ModpkgChunk {
    path_hash: u64,

    data_offset: usize,
    compressed_size: usize,
    uncompressed_size: usize,

    compressed_checksum: u64,
    uncompressed_checksum: u64,

    path_index: u32,
    wad_paths_index: u32,
    layers_index: u32,
}

impl ModpkgChunk {
    pub fn read(reader: &mut BufReader<impl Read>) -> Result<Self, ModpkgError> {
        let path_hash = reader.read_u64::<LE>()?;

        let data_offset = reader.read_u64::<LE>()?;
        let compressed_size = reader.read_u64::<LE>()?;
        let uncompressed_size = reader.read_u64::<LE>()?;

        let compressed_checksum = reader.read_u64::<LE>()?;
        let uncompressed_checksum = reader.read_u64::<LE>()?;

        let path_index = reader.read_u32::<LE>()?;
        let wad_paths_index = reader.read_u32::<LE>()?;
        let layers_index = reader.read_u32::<LE>()?;
        let _ = reader.read_u32::<LE>()?; // reserved

        Ok(Self {
            path_hash,
            data_offset: data_offset as usize,
            compressed_size: compressed_size as usize,
            uncompressed_size: uncompressed_size as usize,
            compressed_checksum,
            uncompressed_checksum,
            path_index,
            wad_paths_index,
            layers_index,
        })
    }

    pub fn path_hash(&self) -> u64 {
        self.path_hash
    }

    pub fn data_offset(&self) -> usize {
        self.data_offset
    }
    pub fn compressed_size(&self) -> usize {
        self.compressed_size
    }
    pub fn uncompressed_size(&self) -> usize {
        self.uncompressed_size
    }

    pub fn compressed_checksum(&self) -> u64 {
        self.compressed_checksum
    }
    pub fn uncompressed_checksum(&self) -> u64 {
        self.uncompressed_checksum
    }

    pub fn path_index(&self) -> u32 {
        self.path_index
    }
    pub fn wad_paths_index(&self) -> u32 {
        self.wad_paths_index
    }
    pub fn layers_index(&self) -> u32 {
        self.layers_index
    }
}
