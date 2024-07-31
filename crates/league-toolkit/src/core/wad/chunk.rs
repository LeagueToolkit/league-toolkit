use std::io::{BufReader, Read};

use byteorder::{ReadBytesExt as _, LE};
use num_enum::{IntoPrimitive, TryFromPrimitive};

use super::WadError;

#[cfg_attr(feature = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum WadChunkCompression {
    None = 0,
    GZip = 1,
    Satellite = 2,
    Zstd = 3,
    ZstdMulti = 4,
}

#[cfg_attr(feature = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WadChunk {
    pub path_hash: u64,
    pub data_offset: usize,
    pub compressed_size: usize,
    pub uncompressed_size: usize,
    pub compression_type: WadChunkCompression,
    pub is_duplicated: bool,
    pub frame_count: u8,
    pub start_frame: u16,
    pub checksum: u64,
}

impl WadChunk {
    pub(crate) fn read<R: Read>(reader: &mut BufReader<R>) -> Result<WadChunk, WadError> {
        let path_hash = reader.read_u64::<LE>()?;
        let data_offset = reader.read_u32::<LE>()? as usize;
        let compressed_size = reader.read_i32::<LE>()? as usize;
        let uncompressed_size = reader.read_i32::<LE>()? as usize;

        let type_frame_count = reader.read_u8()?;
        let frame_count = type_frame_count >> 4;
        let compression_type = WadChunkCompression::try_from_primitive(type_frame_count & 0xF)
            .expect("failed to read chunk compression");

        let is_duplicated = reader.read_u8()? == 1;
        let start_frame = reader.read_u16::<LE>()?;
        let checksum = reader.read_u64::<LE>()?;

        Ok(WadChunk {
            path_hash,
            data_offset,
            compressed_size,
            uncompressed_size,
            compression_type,
            is_duplicated,
            frame_count,
            start_frame,
            checksum,
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
    pub fn compression_type(&self) -> WadChunkCompression {
        self.compression_type
    }
    pub fn checksum(&self) -> u64 {
        self.checksum
    }
}
