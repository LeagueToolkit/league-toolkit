use std::{
    io::{BufRead as _, BufReader, Read, Seek, Write as _},
    marker::PhantomData,
    mem,
};

use super::ZSTD_MAGIC;

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
    Zstd(zstd::stream::Decoder<'a, BufReader<T>>),
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

                        let mut decoder = zstd::Decoder::with_buffer(reader)
                            .expect("failed to create zstd decoder");

                        // if there's still room in the buffer,
                        // decode some more zstd data
                        let written = match buf.len() > position {
                            true => decoder.read(&mut buf[header_off..])?,
                            false => 0,
                        };
                        println!("written: {written}");
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
        // Since the BufReader is reused between uncompressed/zstd states,
        // we trade off slower worst-case speed of header searching (O(n)),
        // for faster zstd decompression
        // (also in practice there seems to be not much uncompressed data before the zstd
        // starts, so the header search shouldn't actually be much slower )
        zstd::zstd_safe::DCtx::in_size()
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
