use std::{
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    marker::PhantomData,
    mem::{self},
};

use super::{WadChunk, WadChunkCompression, WadError};
use flate2::bufread::GzDecoder;

#[cfg(all(feature = "zstd", feature = "ruzstd"))]
compile_error!("feature \"zstd\" and feature \"ruzstd\" cannot be enabled at the same time");

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

pub enum RawChunkDecoder<'a, T: Read + Seek> {
    Uncompressed(T),
    Gzip(GzDecoder<BufReader<T>>),
    #[cfg(feature = "zstd")]
    Zstd(zstd::stream::Decoder<'a, BufReader<T>>),
    #[cfg(feature = "ruzstd")]
    Zstd(ruzstd::decoding::StreamingDecoder<T, ruzstd::decoding::FrameDecoder>),
    ZstdMulti(ZstdMultiDecoder<'a, T>),
}

impl<T: Read + Seek> RawChunkDecoder<'_, T> {
    fn new(kind: WadChunkCompression, source: T) -> Self {
        match kind {
            WadChunkCompression::None => RawChunkDecoder::Uncompressed(source),
            WadChunkCompression::GZip => {
                RawChunkDecoder::Gzip(GzDecoder::new(BufReader::new(source)))
            }
            WadChunkCompression::Satellite => todo!(),
            #[cfg(feature = "zstd")]
            WadChunkCompression::Zstd => RawChunkDecoder::Zstd(
                zstd::Decoder::new(source).expect("failed to create zstd decoder"),
            ),
            #[cfg(feature = "ruzstd")]
            WadChunkCompression::Zstd => RawChunkDecoder::Zstd(
                ruzstd::decoding::StreamingDecoder::new(source)
                    .expect("failed to create ruzstd decoder"),
            ),
            WadChunkCompression::ZstdMulti => {
                RawChunkDecoder::ZstdMulti(ZstdMultiDecoder::new(source))
            }
        }
    }
}

impl<T: Read + Seek> Read for RawChunkDecoder<'_, T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        match self {
            RawChunkDecoder::Uncompressed(src) => src.read(buf),
            RawChunkDecoder::Gzip(dec) => dec.read(buf),
            RawChunkDecoder::Zstd(dec) => dec.read(buf),
            RawChunkDecoder::ZstdMulti(dec) => dec.read(buf),
        }
    }
}

#[derive(Default)]
enum MultiState<'a, T: Read + Seek> {
    #[default]
    Invalid,
    Uncompressed {
        position: usize,
        magic_idx: usize,
        reader: BufReader<T>,
        _phantom: PhantomData<&'a ()>,
    },
    #[cfg(feature = "zstd")]
    Zstd(zstd::stream::Decoder<'a, BufReader<T>>),
    #[cfg(feature = "ruzstd")]
    Zstd(ruzstd::decoding::StreamingDecoder<BufReader<T>, ruzstd::decoding::FrameDecoder>),
}

#[cfg(feature = "zstd")]
#[inline(always)]
fn make_zstd_decoder<'a, R: BufRead>(reader: R) -> zstd::Decoder<'a, R> {
    zstd::Decoder::with_buffer(reader).expect("failed to create zstd decoder")
}

#[cfg(feature = "ruzstd")]
#[inline(always)]
fn make_zstd_decoder<R: Read>(
    reader: R,
) -> ruzstd::decoding::StreamingDecoder<R, ruzstd::decoding::FrameDecoder> {
    ruzstd::decoding::StreamingDecoder::new(reader).expect("failed to create ruzstd decoder")
}

impl<T: Read + Seek> MultiState<'_, T> {
    #[inline]
    fn read(self, mut buf: &mut [u8]) -> std::io::Result<(Self, usize)> {
        Ok(match self {
            MultiState::Invalid => unreachable!("ZstdMulti reader entered an invalid state!"), // TODO: make this unreachable_unchecked?
            MultiState::Uncompressed {
                mut reader,
                mut position,
                mut magic_idx,
                _phantom,
            } => {
                let inner_buf = reader.fill_buf()?;
                let mut found_magic = false;

                for byte in inner_buf {
                    if magic_idx == ZSTD_MAGIC.len() {
                        found_magic = true;
                        break;
                    }
                    magic_idx = match *byte == ZSTD_MAGIC[magic_idx] {
                        true => magic_idx + 1,
                        false => 0,
                    };
                    position += 1;
                }

                match found_magic {
                    true => {
                        // zstd header found, we consume all bytes before the header,
                        // and become a zstd decoder
                        let header_off = position - ZSTD_MAGIC.len();
                        buf.write_all(&inner_buf[..header_off])?;
                        reader.consume(header_off);

                        let mut decoder = make_zstd_decoder(reader);

                        // if there's still room in the buffer,
                        // decode some more zstd data
                        let written = match buf.len() > header_off {
                            true => decoder.read(buf)?,
                            false => 0,
                        };
                        (Self::Zstd(decoder), written)
                    }
                    false => {
                        // we can safely consume up to any partial header match we might have
                        // e.g: magic_idx is 2, since we found 2 of the bytes in our buffer:
                        //   [??, ??, ??, 0, 1]
                        // we know any partial header will be at the end of the buffer,
                        // so len - idx works
                        let safe_bytes = inner_buf.len() - magic_idx;
                        buf.write_all(&inner_buf[..safe_bytes])?;
                        reader.consume(safe_bytes);

                        (
                            Self::Uncompressed {
                                reader,
                                position,
                                magic_idx,
                                _phantom,
                            },
                            safe_bytes,
                        )
                    }
                }
            }
            MultiState::Zstd(mut decoder) => {
                let written = decoder.read(buf)?;
                (MultiState::Zstd(decoder), written)
            }
        })
    }
}

pub struct ZstdMultiDecoder<'a, T: Read + Seek> {
    state: MultiState<'a, T>,
}

const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

impl<T: Read + Seek> ZstdMultiDecoder<'_, T> {
    pub fn new(source: T) -> Self {
        Self {
            state: MultiState::Uncompressed {
                position: 0,
                magic_idx: 0,
                reader: BufReader::with_capacity(Self::buffer_size(), source),
                _phantom: PhantomData,
            },
        }
    }

    #[inline(always)]
    fn buffer_size() -> usize {
        #[cfg(feature = "zstd")]
        {
            // Since the BufReader is reused between uncompressed/zstd states,
            // we trade off slower worst-case speed of header searching (O(n)),
            // for faster zstd decompression
            // (also in practice there seems to be not much uncompressed data before the zstd
            // starts, so the header search shouldn't actually be much slower )
            zstd::zstd_safe::DCtx::in_size()
        }
        #[cfg(feature = "ruzstd")]
        {
            // TODO: figure out the best size here
            512
        }
    }
}

impl<T: Read + Seek> Read for ZstdMultiDecoder<'_, T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let state = mem::take(&mut self.state);
        let (state, written) = state.read(buf)?;
        self.state = state;
        Ok(written)
    }
}
