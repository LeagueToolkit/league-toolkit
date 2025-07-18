use std::io::{Read, Seek, SeekFrom};

use crate::WadError;

use super::{WadChunk, WadChunkCompression};

mod raw;
pub use raw::*;

mod zstd;

pub struct ChunkDecoder<'a, T: Read + Seek> {
    size: usize,
    position: usize,
    inner: RawChunkDecoder<'a, &'a mut T>,
}

impl<'a, T: Read + Seek> ChunkDecoder<'a, T> {
    /// # Safety
    /// The raw decoder does not know or care about the uncompressed size of the chunk, which means you can
    /// easily "over-read" the underlying buffer.
    ///
    /// Make sure you never read more than [`WadChunk::uncompressed_size`] when using the raw decoder.
    pub unsafe fn raw_decoder(&mut self) -> &mut RawChunkDecoder<'a, &'a mut T> {
        &mut self.inner
    }

    pub fn compression_type(&self) -> WadChunkCompression {
        match &self.inner {
            RawChunkDecoder::Uncompressed(_) => WadChunkCompression::None,
            RawChunkDecoder::Gzip(_) => WadChunkCompression::GZip,
            RawChunkDecoder::Zstd(_) => WadChunkCompression::Zstd,
            RawChunkDecoder::ZstdMulti(_) => WadChunkCompression::ZstdMulti,
        }
    }

    pub fn new(chunk: &WadChunk, source: &'a mut T) -> Result<Self, WadError> {
        source.seek(SeekFrom::Start(chunk.data_offset() as _))?;
        let inner = RawChunkDecoder::new(chunk.compression_type(), source);
        Ok(Self {
            size: chunk.uncompressed_size(),
            position: 0,
            inner,
        })
    }
}

impl<T: Read + Seek> Read for ChunkDecoder<'_, T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let len = buf.len();
        let buf = &mut buf[..len.min(self.size - self.position)];

        let bytes = self.inner.read(buf)?;
        self.position += bytes;
        Ok(bytes)
    }
}
