use std::{
    fmt,
    io::{Cursor, Read},
};

use super::{SubchunkToc, WadChunk, WadChunkCompression, WadError, WadSubchunk};
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

/// Decompresses a `ZstdMulti` chunk by its subchunk records.
///
/// Each record's compressed bytes are the next run of `raw_data`: a raw copy
/// when its sizes are equal, one zstd frame otherwise. [`decompress_raw`]
/// instead guesses the layout by scanning for the first zstd magic, which a
/// raw subchunk holding those four bytes defeats.
///
/// # Errors
///
/// Fails when the records do not fit `raw_data`, when their uncompressed
/// sizes do not sum to `uncompressed_size`, or when a subchunk does not
/// decode.
pub fn decompress_subchunked(
    raw_data: &[u8],
    subchunks: &[WadSubchunk],
    uncompressed_size: usize,
) -> Result<Box<[u8]>, WadError> {
    let mut data = vec![0; uncompressed_size];
    let mut read = 0;
    let mut written = 0;
    for subchunk in subchunks {
        let (input, output) =
            subchunk_slices(raw_data, &mut data, subchunk, &mut read, &mut written)?;
        if subchunk.is_raw() {
            output.copy_from_slice(input);
        } else {
            zstd_reader(input)?.read_exact(output)?;
        }
    }
    if written != uncompressed_size {
        return Err(WadError::DecompressionFailure {
            reason: format!(
                "subchunks decompressed to {written} bytes, expected {uncompressed_size}"
            ),
        });
    }
    Ok(data.into_boxed_slice())
}

/// One subchunk's run of the input and its slot in the output.
fn subchunk_slices<'d>(
    raw_data: &'d [u8],
    data: &'d mut [u8],
    subchunk: &WadSubchunk,
    read: &mut usize,
    written: &mut usize,
) -> Result<(&'d [u8], &'d mut [u8]), WadError> {
    let input = raw_data
        .get(*read..*read + subchunk.compressed_size as usize)
        .ok_or_else(|| WadError::DecompressionFailure {
            reason: String::from("subchunk records overrun the chunk's compressed data"),
        })?;
    let output = data
        .get_mut(*written..*written + subchunk.uncompressed_size as usize)
        .ok_or_else(|| WadError::DecompressionFailure {
            reason: String::from("subchunk records overrun the chunk's uncompressed size"),
        })?;
    *read += subchunk.compressed_size as usize;
    *written += subchunk.uncompressed_size as usize;
    Ok((input, output))
}

/// Decompresses at most `max_len` bytes from the start of a chunk's raw data.
///
/// `raw_data` may be a prefix of the chunk's raw bytes. As long as it holds
/// the first compressed block, the first bytes decode. That is all a read of
/// a chunk's magic wants, without the rest of the chunk. A prefix that cuts
/// the first block short fails to decode.
///
/// For [`WadChunkCompression::None`], the prefix of the input comes back as is.
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

    /// Decompress a whole chunk, by its subchunk records when `toc` holds them.
    ///
    /// The records settle a `ZstdMulti` chunk's layout exactly; without them
    /// [`decompress`](Self::decompress) falls back to scanning for the first
    /// zstd frame.
    pub fn decompress_chunk(
        &mut self,
        raw_data: &[u8],
        chunk: &WadChunk,
        toc: Option<&SubchunkToc>,
    ) -> Result<Box<[u8]>, WadError> {
        match toc.and_then(|toc| toc.subchunks_of(chunk)) {
            Some(subchunks) => {
                self.decompress_subchunked(raw_data, subchunks, chunk.uncompressed_size)
            }
            None => self.decompress(raw_data, chunk.compression_type, chunk.uncompressed_size),
        }
    }

    /// Decompress a `ZstdMulti` chunk by its subchunk records.
    ///
    /// The same as [`decompress_subchunked`], with the decoder kept.
    pub fn decompress_subchunked(
        &mut self,
        raw_data: &[u8],
        subchunks: &[WadSubchunk],
        uncompressed_size: usize,
    ) -> Result<Box<[u8]>, WadError> {
        #[cfg(feature = "zstd")]
        {
            let mut data = vec![0; uncompressed_size];
            let mut read = 0;
            let mut written = 0;
            for subchunk in subchunks {
                let (input, output) =
                    subchunk_slices(raw_data, &mut data, subchunk, &mut read, &mut written)?;
                if subchunk.is_raw() {
                    output.copy_from_slice(input);
                } else {
                    let decoded = self.stream_into(input, output)?;
                    if decoded != subchunk.uncompressed_size as usize {
                        return Err(WadError::DecompressionFailure {
                            reason: format!(
                                "subchunk decompressed to {decoded} bytes, expected {}",
                                subchunk.uncompressed_size
                            ),
                        });
                    }
                }
            }
            if written != uncompressed_size {
                return Err(WadError::DecompressionFailure {
                    reason: format!(
                        "subchunks decompressed to {written} bytes, expected {uncompressed_size}"
                    ),
                });
            }
            Ok(data.into_boxed_slice())
        }
        #[cfg(not(feature = "zstd"))]
        decompress_subchunked(raw_data, subchunks, uncompressed_size)
    }

    /// Decompress at most `max_len` bytes from the start of a chunk, by its
    /// subchunk records when `toc` holds them.
    ///
    /// The same as [`decompress_prefix`](Self::decompress_prefix) otherwise.
    pub fn decompress_chunk_prefix(
        &mut self,
        raw_data: &[u8],
        chunk: &WadChunk,
        toc: Option<&SubchunkToc>,
        max_len: usize,
    ) -> Result<Vec<u8>, WadError> {
        match toc.and_then(|toc| toc.subchunks_of(chunk)) {
            Some(subchunks) => self.decompress_subchunked_prefix(raw_data, subchunks, max_len),
            None => self.decompress_prefix(raw_data, chunk.compression_type, max_len),
        }
    }

    /// Decompress at most `max_len` bytes from the start of a `ZstdMulti`
    /// chunk, by its subchunk records.
    ///
    /// `raw_data` may be a prefix of the chunk's raw bytes; input that ends
    /// mid-subchunk comes back as a shorter result rather than an error.
    pub fn decompress_subchunked_prefix(
        &mut self,
        raw_data: &[u8],
        subchunks: &[WadSubchunk],
        max_len: usize,
    ) -> Result<Vec<u8>, WadError> {
        let mut data = Vec::with_capacity(max_len);
        let mut read = 0;
        for subchunk in subchunks {
            if data.len() >= max_len || read >= raw_data.len() {
                break;
            }
            let end = read + subchunk.compressed_size as usize;
            let input = &raw_data[read..end.min(raw_data.len())];
            let want = (max_len - data.len()).min(subchunk.uncompressed_size as usize);
            if subchunk.is_raw() {
                data.extend_from_slice(&input[..want.min(input.len())]);
            } else {
                let decoded = self.zstd_prefix_of(input, want)?;
                let cut_short = decoded.len() < want;
                data.extend(decoded);
                if cut_short {
                    break;
                }
            }
            if end > raw_data.len() {
                break;
            }
            read = end;
        }
        Ok(data)
    }

    /// At most `want` decoded bytes from the start of `input`'s zstd frames.
    #[cfg(feature = "zstd")]
    fn zstd_prefix_of(&mut self, input: &[u8], want: usize) -> Result<Vec<u8>, WadError> {
        let mut buffer = vec![0; want];
        let decoded = self.stream_into(input, &mut buffer)?;
        buffer.truncate(decoded);
        Ok(buffer)
    }

    #[cfg(not(feature = "zstd"))]
    fn zstd_prefix_of(&mut self, input: &[u8], want: usize) -> Result<Vec<u8>, WadError> {
        read_prefix(zstd_reader(input)?, want)
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

    /// Decode frames from `input` into `output` until one of them ends.
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

#[cfg(all(test, feature = "zstd"))]
mod tests {
    use super::*;

    fn frame(data: &[u8]) -> Vec<u8> {
        zstd::encode_all(Cursor::new(data), 3).expect("encoding a test frame cannot fail")
    }

    fn subchunk(compressed: &[u8], uncompressed_size: usize) -> WadSubchunk {
        WadSubchunk {
            compressed_size: compressed.len() as u32,
            uncompressed_size: uncompressed_size as u32,
            checksum: 0,
        }
    }

    /// A raw run and two frames, with the raw run holding the zstd magic, so a
    /// scan for the first frame starts inside it and decodes garbage.
    fn chunk_with_magic_in_raw() -> (Vec<u8>, Vec<WadSubchunk>, Vec<u8>) {
        let first = b"raw (\xb5/\xfdbytes with a fake frame start".to_vec();
        let second = b"the middle subchunk, compressed".repeat(20);
        let third = b"the last subchunk, compressed too".repeat(20);
        let (second_frame, third_frame) = (frame(&second), frame(&third));

        let subchunks = vec![
            subchunk(&first, first.len()),
            subchunk(&second_frame, second.len()),
            subchunk(&third_frame, third.len()),
        ];
        let raw: Vec<u8> = [first.as_slice(), &second_frame, &third_frame].concat();
        let expected: Vec<u8> = [first.as_slice(), &second, &third].concat();
        (raw, subchunks, expected)
    }

    #[test]
    fn subchunked_decodes_a_raw_run_holding_the_magic() {
        let (raw, subchunks, expected) = chunk_with_magic_in_raw();

        let data = decompress_subchunked(&raw, &subchunks, expected.len()).unwrap();
        assert_eq!(&data[..], &expected[..]);

        let kept = ChunkDecoder::new()
            .decompress_subchunked(&raw, &subchunks, expected.len())
            .unwrap();
        assert_eq!(&kept[..], &expected[..]);

        /* The magic scan starts at the fake frame start and cannot decode. */
        decompress_raw(&raw, WadChunkCompression::ZstdMulti, expected.len()).unwrap_err();
    }

    #[test]
    fn subchunked_decodes_a_raw_middle_subchunk() {
        let first = b"first, compressed".repeat(30);
        let middle = b"a raw middle".to_vec();
        let last = b"last, compressed".repeat(30);
        let (first_frame, last_frame) = (frame(&first), frame(&last));

        let subchunks = vec![
            subchunk(&first_frame, first.len()),
            subchunk(&middle, middle.len()),
            subchunk(&last_frame, last.len()),
        ];
        let raw: Vec<u8> = [first_frame.as_slice(), &middle, &last_frame].concat();
        let expected: Vec<u8> = [first.as_slice(), &middle, &last].concat();

        let data = ChunkDecoder::new()
            .decompress_subchunked(&raw, &subchunks, expected.len())
            .unwrap();
        assert_eq!(&data[..], &expected[..]);
    }

    #[test]
    fn subchunked_rejects_records_that_overrun_the_data() {
        let subchunks = vec![WadSubchunk {
            compressed_size: 100,
            uncompressed_size: 200,
            checksum: 0,
        }];
        decompress_subchunked(&[0u8; 10], &subchunks, 200).unwrap_err();
        ChunkDecoder::new()
            .decompress_subchunked(&[0u8; 10], &subchunks, 200)
            .unwrap_err();
    }

    #[test]
    fn subchunked_rejects_sizes_that_do_not_sum() {
        let content = b"some content".repeat(10);
        let encoded = frame(&content);
        let subchunks = vec![subchunk(&encoded, content.len())];
        decompress_subchunked(&encoded, &subchunks, content.len() + 1).unwrap_err();
    }

    #[test]
    fn subchunked_prefix_reads_the_first_bytes() {
        let (raw, subchunks, expected) = chunk_with_magic_in_raw();
        let mut decoder = ChunkDecoder::new();

        /* From inside the raw run. */
        let head = decoder
            .decompress_subchunked_prefix(&raw, &subchunks, 8)
            .unwrap();
        assert_eq!(&head[..], &expected[..8]);

        /* Across the raw run into the first frame. */
        let first_len = subchunks[0].uncompressed_size as usize;
        let head = decoder
            .decompress_subchunked_prefix(&raw, &subchunks, first_len + 16)
            .unwrap();
        assert_eq!(&head[..], &expected[..first_len + 16]);
    }

    #[test]
    fn subchunked_prefix_of_cut_short_input_comes_back_shorter() {
        let (raw, subchunks, expected) = chunk_with_magic_in_raw();
        let first_len = subchunks[0].uncompressed_size as usize;

        /* The whole raw run, and the first frame cut mid-block. */
        let head = ChunkDecoder::new()
            .decompress_subchunked_prefix(&raw[..first_len + 4], &subchunks, expected.len())
            .unwrap();
        assert!(head.len() >= first_len);
        assert_eq!(&head[..], &expected[..head.len()]);
    }
}
