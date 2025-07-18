mod read;
mod write;

mod decoder;
pub use decoder::*;

mod compression;
pub use compression::*;

use crate::WadError;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// A single wad chunk
pub struct WadChunk {
    pub(crate) path_hash: u64,
    pub(crate) data_offset: usize,
    pub(crate) compressed_size: usize,
    pub(crate) uncompressed_size: usize,
    pub(crate) compression_type: WadChunkCompression,
    pub(crate) is_duplicated: bool,
    pub(crate) frame_count: u8,
    pub(crate) start_frame: u32,
    pub(crate) checksum: u64,
}

impl WadChunk {
    pub fn decoder<'a, T: std::io::Read + std::io::Seek>(
        &self,
        source: &'a mut T,
    ) -> Result<ChunkDecoder<'a, T>, WadError> {
        ChunkDecoder::new(self, source)
    }

    #[must_use]
    #[inline(always)]
    pub fn path_hash(&self) -> u64 {
        self.path_hash
    }

    #[must_use]
    #[inline(always)]
    pub fn data_offset(&self) -> usize {
        self.data_offset
    }

    #[must_use]
    #[inline(always)]
    pub fn compressed_size(&self) -> usize {
        self.compressed_size
    }

    #[must_use]
    #[inline(always)]
    pub fn uncompressed_size(&self) -> usize {
        self.uncompressed_size
    }

    #[must_use]
    #[inline(always)]
    pub fn compression_type(&self) -> WadChunkCompression {
        self.compression_type
    }

    #[must_use]
    #[inline(always)]
    pub fn checksum(&self) -> u64 {
        self.checksum
    }
}
