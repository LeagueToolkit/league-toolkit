//! Wad file handling

mod builder;
mod chunk;
mod decoder;
mod error;

pub use builder::*;
pub use chunk::*;
pub use decoder::*;
pub use error::*;

use std::{
    collections::HashMap,
    io::{BufReader, Read, Seek, SeekFrom},
};

use byteorder::{ReadBytesExt as _, LE};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
/// A wad file
pub struct Wad<TSource: Read + Seek> {
    chunks: HashMap<u64, WadChunk>,
    #[cfg_attr(feature = "serde", serde(skip))]
    source: TSource,
}

impl<TSource: Read + Seek> Wad<TSource> {
    pub fn chunks(&self) -> &HashMap<u64, WadChunk> {
        &self.chunks
    }

    pub fn mount(mut source: TSource) -> Result<Wad<TSource>, WadError> {
        let mut reader = BufReader::new(&mut source);

        // 0x5752 = "RW"
        let magic = reader.read_u16::<LE>()?;
        if magic != 0x5752 {
            return Err(WadError::InvalidHeader {
                expected: String::from("RW"),
                actual: format!("0x{:x}", magic),
            });
        }

        let major = reader.read_u8()?;
        let minor = reader.read_u8()?;
        if major > 3 {
            return Err(WadError::InvalidVersion { major, minor });
        }

        if major == 2 {
            let _ecdsa_length = reader.seek(SeekFrom::Current(1))?;
            let _ecdsa_signature = reader.seek(SeekFrom::Current(83))?;
            let _data_checksum = reader.seek(SeekFrom::Current(8))?;
        } else if major == 3 {
            let _ecdsa_signature = reader.seek(SeekFrom::Current(256))?;
            let _data_checksum = reader.seek(SeekFrom::Current(8))?;
        }

        if major == 1 || major == 2 {
            let _toc_start_offset = reader.seek(SeekFrom::Current(2))?;
            let _toc_chunk_size = reader.seek(SeekFrom::Current(2))?;
        }

        let chunk_count = reader.read_i32::<LE>()? as usize;
        let mut chunks = HashMap::<u64, WadChunk>::with_capacity(chunk_count);
        for _ in 0..chunk_count {
            let chunk = WadChunk::read_v3_1(&mut reader)?;
            chunks
                .insert(chunk.path_hash(), chunk)
                .map_or(Ok(()), |chunk| {
                    Err(WadError::DuplicateChunk {
                        path_hash: chunk.path_hash(),
                    })
                })?;
        }

        Ok(Wad { chunks, source })
    }

    pub fn decode(&mut self) -> (WadDecoder<'_, TSource>, &HashMap<u64, WadChunk>) {
        (
            WadDecoder {
                source: &mut self.source,
            },
            &self.chunks,
        )
    }
}
