use crate::ModpkgCompression;
use binrw::binrw;

#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ModpkgChunk {
    pub path_hash: u64,

    pub data_offset: u64,
    pub compression: ModpkgCompression,
    pub compressed_size: u64,
    pub uncompressed_size: u64,

    pub compressed_checksum: u64,
    pub uncompressed_checksum: u64,

    pub path_index: u32,
    pub wad_paths_index: u32,
    pub layer_index: u32,
}

impl ModpkgChunk {
    pub fn path_hash(&self) -> u64 {
        self.path_hash
    }

    pub fn data_offset(&self) -> u64 {
        self.data_offset
    }
    pub fn compression(&self) -> ModpkgCompression {
        self.compression
    }
    pub fn compressed_size(&self) -> u64 {
        self.compressed_size
    }
    pub fn uncompressed_size(&self) -> u64 {
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
    pub fn layer_index(&self) -> u32 {
        self.layer_index
    }
}
