use std::io::{self, Read, Seek};

use binrw::{BinRead, BinWrite};
use flate2::read::GzDecoder;
use memchr::memmem;
use num_enum::{IntoPrimitive, TryFromPrimitive};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
#[brw(little, repr = u8)]
pub enum EntryKind {
    /// Uncompressed
    None = 0,
    /// GZip compressed data
    GZip = 1,
    /// Satellite compressed
    Satellite = 2,
    /// zstd compressed
    Zstd = 3,
    /// zstd compressed data, with some uncompressed data before it
    ZstdMulti = 4,
}

const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

impl EntryKind {
    pub fn decode(
        &self,
        data: &mut (impl Read + Seek),
        compressed_size: usize,
        uncompressed_size: usize,
    ) -> io::Result<Vec<u8>> {
        let mut out = vec![0; uncompressed_size];
        match self {
            EntryKind::None => {
                data.read_exact(&mut out)?;
            }
            EntryKind::GZip => {
                GzDecoder::new(data).read_exact(&mut out)?;
            }
            EntryKind::Satellite => todo!(),
            EntryKind::Zstd => {
                zstd::Decoder::new(data)
                    .expect("failed to create zstd decoder")
                    .read_exact(&mut out)?;
            }
            EntryKind::ZstdMulti => {
                let mut search = vec![0; compressed_size];
                data.read_exact(&mut search)?;
                let magic_off =
                    memmem::find(&search, &ZSTD_MAGIC).expect("could not find zstd magic");
                for (i, val) in search[..magic_off].iter().enumerate() {
                    out[i] = *val;
                }

                zstd::Decoder::new(&mut &search[magic_off..])
                    .expect("failed to create zstd decoder")
                    .read_exact(&mut out[magic_off..])?;
            }
        }
        Ok(out)
    }
}
