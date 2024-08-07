use std::io::{Read, Seek, SeekFrom};

use super::{WadChunk, WadChunkCompression, WadError};
use flate2::read::GzDecoder;
use memchr::memmem;

const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

#[cfg(all(feature = "zstd", feature = "ruzstd"))]
compile_error!("feature \"zstd\" and feature \"ruzstd\" cannot be enabled at the same time");

pub struct WadDecoder<'wad, TSource: Read + Seek> {
    pub(crate) source: &'wad mut TSource,
}

impl<'wad, TSource> WadDecoder<'wad, TSource>
where
    TSource: Read + Seek,
{
    pub fn load_chunk_raw(&mut self, chunk: &WadChunk) -> Result<Box<[u8]>, WadError> {
        let mut data = vec![0; chunk.compressed_size];

        self.source
            .seek(SeekFrom::Start(chunk.data_offset as u64))?;
        self.source.read_exact(&mut data)?;

        Ok(data.into_boxed_slice())
    }
    pub fn load_chunk_decompressed(&mut self, chunk: &WadChunk) -> Result<Box<[u8]>, WadError> {
        match chunk.compression_type {
            WadChunkCompression::None => self.load_chunk_raw(chunk),
            WadChunkCompression::GZip => self.decode_gzip_chunk(chunk),
            WadChunkCompression::Satellite => Err(WadError::Other(String::from(
                "satellite chunks are not supported",
            ))),
            WadChunkCompression::Zstd => self.decode_zstd_chunk(chunk),
            WadChunkCompression::ZstdMulti => self.decode_zstd_multi_chunk(chunk),
            _ => todo!(),
        }
    }

    fn decode_gzip_chunk(&mut self, chunk: &WadChunk) -> Result<Box<[u8]>, WadError> {
        self.source
            .seek(SeekFrom::Start(chunk.data_offset as u64))?;

        let mut data = vec![0; chunk.uncompressed_size];
        log::debug!("decoding gzip chunk...");
        GzDecoder::new(&mut self.source).read_exact(&mut data)?;

        Ok(data.into_boxed_slice())
    }
    fn decode_zstd_chunk(&mut self, chunk: &WadChunk) -> Result<Box<[u8]>, WadError> {
        self.source
            .seek(SeekFrom::Start(chunk.data_offset as u64))?;

        let mut data: Vec<u8> = vec![0; chunk.uncompressed_size];

        log::debug!("decoding zstd chunk...");
        #[cfg(feature = "zstd")]
        {
            zstd::Decoder::new(&mut self.source)
                .expect("failed to create zstd decoder")
                .read_exact(&mut data)?;
        }
        #[cfg(feature = "ruzstd")]
        {
            ruzstd::StreamingDecoder::new(&mut self.source)
                .expect("failed to create ruzstd decoder")
                .read_exact(&mut data)?;
        }

        Ok(data.into_boxed_slice())
    }
    fn decode_zstd_multi_chunk(&mut self, chunk: &WadChunk) -> Result<Box<[u8]>, WadError> {
        let raw_data = self.load_chunk_raw(chunk)?;
        let mut data: Vec<u8> = vec![0; chunk.uncompressed_size];

        let zstd_magic_offset =
            memmem::find(&raw_data, &ZSTD_MAGIC).ok_or(WadError::DecompressionFailure {
                path_hash: chunk.path_hash,
                reason: String::from("failed to find zstd magic"),
            })?;

        // copy raw uncompressed data which exists before first zstd frame
        for (i, value) in raw_data[0..zstd_magic_offset].iter().enumerate() {
            data[i] = *value;
        }

        // seek to start of first zstd frame
        self.source.seek(SeekFrom::Start(
            (chunk.data_offset + zstd_magic_offset) as u64,
        ))?;

        log::debug!(
            "decoding zstd multi chunk...\ndata_off: {}\nmagic_off: {zstd_magic_offset}",
            chunk.data_offset
        );
        // decode zstd data
        #[cfg(feature = "zstd")]
        {
            zstd::Decoder::new(&mut self.source)
                .expect("failed to create zstd decoder")
                .read_exact(&mut data[zstd_magic_offset..])?;
        }
        #[cfg(feature = "ruzstd")]
        {
            ruzstd::StreamingDecoder::new(&mut self.source)
                .expect("failed to create ruzstd decoder")
                .read(&mut data[zstd_magic_offset..])?;
        }

        Ok(data.into_boxed_slice())
    }
}
