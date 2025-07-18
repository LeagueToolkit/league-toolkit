mod multi;
pub use multi::*;

pub const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

#[cfg(all(feature = "zstd", feature = "ruzstd"))]
compile_error!("feature \"zstd\" and feature \"ruzstd\" cannot be enabled at the same time");

#[cfg(feature = "zstd")]
use std::io::BufRead;

#[cfg(feature = "zstd")]
#[inline(always)]
pub fn make_zstd_decoder<'a, R: BufRead>(reader: R) -> zstd::Decoder<'a, R> {
    zstd::Decoder::with_buffer(reader).expect("failed to create zstd decoder")
}

#[cfg(feature = "ruzstd")]
use std::io::Read;

#[cfg(feature = "ruzstd")]
#[inline(always)]
pub fn make_zstd_decoder<R: Read>(
    reader: R,
) -> ruzstd::decoding::StreamingDecoder<R, ruzstd::decoding::FrameDecoder> {
    ruzstd::decoding::StreamingDecoder::new(reader).expect("failed to create ruzstd decoder")
}
