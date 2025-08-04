use std::io::{BufReader, Read, Seek};

use flate2::read::GzDecoder;

use super::{zstd::ZstdMultiDecoder, WadChunkCompression};

pub enum RawChunkDecoder<'a, T: Read + Seek> {
    Uncompressed(T),
    Gzip(GzDecoder<BufReader<T>>),
    Zstd(zstd::stream::Decoder<'a, BufReader<T>>),
    ZstdMulti(ZstdMultiDecoder<'a, T>),
}

impl<T: Read + Seek> RawChunkDecoder<'_, T> {
    pub(crate) fn new(kind: WadChunkCompression, source: T) -> Self {
        match kind {
            WadChunkCompression::None => RawChunkDecoder::Uncompressed(source),
            WadChunkCompression::GZip => {
                RawChunkDecoder::Gzip(GzDecoder::new(BufReader::new(source)))
            }
            WadChunkCompression::Satellite => todo!(),
            WadChunkCompression::Zstd => RawChunkDecoder::Zstd(
                zstd::Decoder::new(source).expect("failed to create zstd decoder"),
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
