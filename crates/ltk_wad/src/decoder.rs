use std::{
    fmt,
    io::{Cursor, Read},
};

use super::{WadChunkCompression, WadError};
use flate2::read::GzDecoder;
use memchr::memmem;

const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

#[cfg(all(feature = "zstd", feature = "ruzstd"))]
compile_error!("feature \"zstd\" and feature \"ruzstd\" cannot be enabled at the same time");

/// Decompresses raw chunk data that has already been read from a WAD source.
///
/// This enables a two-phase parallel workflow:
/// 1. Read raw compressed bytes sequentially via [`crate::Wad::load_chunk_raw`]
/// 2. Decompress in parallel using this function (e.g. with rayon)
///
/// For [`WadChunkCompression::None`], the input data is returned as-is.
pub fn decompress_raw(
    raw_data: &[u8],
    compression: WadChunkCompression,
    uncompressed_size: usize,
) -> Result<Box<[u8]>, WadError> {
    match compression {
        WadChunkCompression::None => Ok(raw_data.into()),
        WadChunkCompression::GZip => decompress_gzip(raw_data, uncompressed_size),
        WadChunkCompression::Satellite => Err(WadError::Other(String::from(
            "satellite chunks are not supported",
        ))),
        WadChunkCompression::Zstd => decompress_zstd(raw_data, uncompressed_size),
        WadChunkCompression::ZstdMulti => decompress_zstd_multi(raw_data, uncompressed_size),
    }
}

fn decompress_gzip(raw_data: &[u8], uncompressed_size: usize) -> Result<Box<[u8]>, WadError> {
    let mut data = vec![0; uncompressed_size];
    GzDecoder::new(Cursor::new(raw_data)).read_exact(&mut data)?;
    Ok(data.into_boxed_slice())
}

fn decompress_zstd(raw_data: &[u8], uncompressed_size: usize) -> Result<Box<[u8]>, WadError> {
    let mut data = vec![0; uncompressed_size];

    #[cfg(feature = "zstd")]
    {
        zstd::Decoder::new(Cursor::new(raw_data))
            .expect("failed to create zstd decoder")
            .read_exact(&mut data)?;
    }
    #[cfg(feature = "ruzstd")]
    {
        ruzstd::decoding::StreamingDecoder::new(Cursor::new(raw_data))
            .expect("failed to create ruzstd decoder")
            .read_exact(&mut data)?;
    }

    Ok(data.into_boxed_slice())
}

fn decompress_zstd_multi(raw_data: &[u8], uncompressed_size: usize) -> Result<Box<[u8]>, WadError> {
    let mut data = vec![0; uncompressed_size];

    let zstd_magic_offset =
        memmem::find(raw_data, &ZSTD_MAGIC).ok_or(WadError::DecompressionFailure {
            reason: String::from("failed to find zstd magic"),
        })?;

    // copy raw uncompressed data which exists before first zstd frame
    data[..zstd_magic_offset].copy_from_slice(&raw_data[..zstd_magic_offset]);

    // decode zstd data from the magic offset onward
    let zstd_data = &raw_data[zstd_magic_offset..];

    #[cfg(feature = "zstd")]
    {
        zstd::Decoder::new(Cursor::new(zstd_data))
            .expect("failed to create zstd decoder")
            .read_exact(&mut data[zstd_magic_offset..])?;
    }
    #[cfg(feature = "ruzstd")]
    {
        ruzstd::decoding::StreamingDecoder::new(Cursor::new(zstd_data))
            .expect("failed to create ruzstd decoder")
            .read(&mut data[zstd_magic_offset..])?;
    }

    Ok(data.into_boxed_slice())
}

/// Decompresses at most `max_len` bytes from the start of a chunk's raw data.
///
/// `raw_data` may be a prefix of the chunk's raw bytes. As long as it holds
/// the first compressed block, the first bytes decode, which is what a read
/// of a chunk's magic wants without the rest of the chunk. A prefix that cuts
/// the first block short fails to decode.
///
/// For [`WadChunkCompression::None`], the prefix of the input is returned as is.
pub fn decompress_prefix(
    raw_data: &[u8],
    compression: WadChunkCompression,
    max_len: usize,
) -> Result<Vec<u8>, WadError> {
    match compression {
        WadChunkCompression::None => Ok(raw_data[..raw_data.len().min(max_len)].to_vec()),
        WadChunkCompression::GZip => read_prefix(GzDecoder::new(Cursor::new(raw_data)), max_len),
        WadChunkCompression::Satellite => Err(WadError::Other(String::from(
            "satellite chunks are not supported",
        ))),
        WadChunkCompression::Zstd => read_prefix(zstd_reader(raw_data)?, max_len),
        WadChunkCompression::ZstdMulti => {
            /* The bytes before the first frame are stored raw, so they are the
            prefix until the frame starts. */
            let frame_at = memmem::find(raw_data, &ZSTD_MAGIC).unwrap_or(raw_data.len());
            let mut data = raw_data[..frame_at.min(max_len)].to_vec();
            if data.len() < max_len && frame_at < raw_data.len() {
                let rest = read_prefix(zstd_reader(&raw_data[frame_at..])?, max_len - data.len())?;
                data.extend(rest);
            }
            Ok(data)
        }
    }
}

fn read_prefix(reader: impl Read, max_len: usize) -> Result<Vec<u8>, WadError> {
    let mut data = Vec::with_capacity(max_len);
    reader.take(max_len as u64).read_to_end(&mut data)?;
    Ok(data)
}

#[cfg(feature = "zstd")]
fn zstd_reader(raw_data: &[u8]) -> Result<impl Read + '_, WadError> {
    Ok(zstd::Decoder::new(Cursor::new(raw_data))?)
}

#[cfg(all(feature = "ruzstd", not(feature = "zstd")))]
fn zstd_reader(raw_data: &[u8]) -> Result<impl Read + '_, WadError> {
    ruzstd::decoding::StreamingDecoder::new(Cursor::new(raw_data)).map_err(|error| {
        WadError::DecompressionFailure {
            reason: error.to_string(),
        }
    })
}

#[cfg(not(any(feature = "zstd", feature = "ruzstd")))]
fn zstd_reader(_raw_data: &[u8]) -> Result<std::io::Empty, WadError> {
    Err(WadError::Other(String::from("zstd support is not enabled")))
}

/// A chunk decompressor that keeps its decoder between chunks.
///
/// A zstd decoder is a context of a few hundred kilobytes, and building one
/// costs about as much as decoding a small chunk. One per thread, kept across
/// chunks, pays for it once. [`decompress_raw`] and [`decompress_prefix`]
/// build one per call, which is the right trade for a single chunk.
///
/// Without the `zstd` feature the methods are the free functions.
#[derive(Default)]
pub struct ChunkDecoder {
    #[cfg(feature = "zstd")]
    zstd: Option<zstd::zstd_safe::DCtx<'static>>,
}

impl fmt::Debug for ChunkDecoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChunkDecoder").finish_non_exhaustive()
    }
}

impl ChunkDecoder {
    /// A decoder with nothing built yet. The first zstd chunk builds the context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Decompress a whole chunk. The same as [`decompress_raw`], with the decoder kept.
    pub fn decompress(
        &mut self,
        raw_data: &[u8],
        compression: WadChunkCompression,
        uncompressed_size: usize,
    ) -> Result<Box<[u8]>, WadError> {
        #[cfg(feature = "zstd")]
        match compression {
            WadChunkCompression::Zstd => {
                return self.decompress_zstd(raw_data, 0, uncompressed_size)
            }
            WadChunkCompression::ZstdMulti => {
                let frame_at = memmem::find(raw_data, &ZSTD_MAGIC).ok_or_else(|| {
                    WadError::DecompressionFailure {
                        reason: String::from("failed to find zstd magic"),
                    }
                })?;
                return self.decompress_zstd(raw_data, frame_at, uncompressed_size);
            }
            _ => {}
        }
        decompress_raw(raw_data, compression, uncompressed_size)
    }

    /// Decompress at most `max_len` bytes from the start of a chunk.
    ///
    /// The same as [`decompress_prefix`], with the decoder kept. A prefix that
    /// cuts the first block short comes back shorter than `max_len` rather than
    /// as an error, so a caller can tell it to read more.
    pub fn decompress_prefix(
        &mut self,
        raw_data: &[u8],
        compression: WadChunkCompression,
        max_len: usize,
    ) -> Result<Vec<u8>, WadError> {
        #[cfg(feature = "zstd")]
        match compression {
            WadChunkCompression::Zstd => return self.decompress_zstd_prefix(raw_data, 0, max_len),
            WadChunkCompression::ZstdMulti => {
                let frame_at = memmem::find(raw_data, &ZSTD_MAGIC).unwrap_or(raw_data.len());
                return self.decompress_zstd_prefix(raw_data, frame_at, max_len);
            }
            _ => {}
        }
        decompress_prefix(raw_data, compression, max_len)
    }

    #[cfg(feature = "zstd")]
    fn decompress_zstd(
        &mut self,
        raw_data: &[u8],
        frame_at: usize,
        uncompressed_size: usize,
    ) -> Result<Box<[u8]>, WadError> {
        let mut data = vec![0; uncompressed_size];
        let copied = frame_at.min(uncompressed_size);
        data[..copied].copy_from_slice(&raw_data[..copied]);
        let written = self.stream_into(&raw_data[frame_at..], &mut data[copied..])?;
        if copied + written != uncompressed_size {
            return Err(WadError::DecompressionFailure {
                reason: format!(
                    "decompressed {} bytes, expected {uncompressed_size}",
                    copied + written
                ),
            });
        }
        Ok(data.into_boxed_slice())
    }

    #[cfg(feature = "zstd")]
    fn decompress_zstd_prefix(
        &mut self,
        raw_data: &[u8],
        frame_at: usize,
        max_len: usize,
    ) -> Result<Vec<u8>, WadError> {
        let mut data = vec![0; max_len];
        let copied = frame_at.min(max_len);
        data[..copied].copy_from_slice(&raw_data[..copied]);
        let written = if frame_at < raw_data.len() {
            self.stream_into(&raw_data[frame_at..], &mut data[copied..])?
        } else {
            0
        };
        data.truncate(copied + written);
        Ok(data)
    }

    /// Decode frames from `input` into `output` until one of them runs out.
    ///
    /// Returns the bytes written. Input that ends mid-block is not an error
    /// here, because a prefix read does that on purpose.
    #[cfg(feature = "zstd")]
    fn stream_into(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize, WadError> {
        use zstd::zstd_safe::{InBuffer, OutBuffer, ResetDirective};

        let context = self.zstd.get_or_insert_with(zstd::zstd_safe::DCtx::create);
        context
            .reset(ResetDirective::SessionOnly)
            .map_err(zstd_error)?;

        let mut input = InBuffer::around(input);
        let mut output = OutBuffer::around(output);
        while output.pos() < output.capacity() && input.pos() < input.src.len() {
            let before = (input.pos(), output.pos());
            context
                .decompress_stream(&mut output, &mut input)
                .map_err(zstd_error)?;
            if (input.pos(), output.pos()) == before {
                break;
            }
        }
        Ok(output.pos())
    }
}

#[cfg(feature = "zstd")]
fn zstd_error(code: zstd::zstd_safe::ErrorCode) -> WadError {
    WadError::DecompressionFailure {
        reason: zstd::zstd_safe::get_error_name(code).to_owned(),
    }
}
