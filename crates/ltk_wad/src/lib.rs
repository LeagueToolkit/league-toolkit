//! Wad file handling

mod builder;
mod chunk;
mod decoder;
mod error;
mod file_ext;

pub use builder::*;
pub use chunk::*;
pub use decoder::*;
pub use error::*;
pub use file_ext::*;

use std::{
    collections::HashMap,
    io::{self, BufReader, Read, Seek, SeekFrom, Write},
};

use byteorder::{ReadBytesExt as _, LE};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
/// A wad file
pub struct Wad<TSource: Read + Seek> {
    pub ecdsa_signature: Option<Vec<u8>>,
    pub data_checksum: Option<u64>,

    chunks: HashMap<u64, WadChunk>,

    #[cfg_attr(feature = "serde", serde(skip))]
    source: TSource,
}

impl<TSource: Read + Seek> Wad<TSource> {
    pub fn chunks(&self) -> &HashMap<u64, WadChunk> {
        &self.chunks
    }

    pub fn source(&self) -> &TSource {
        &self.source
    }

    pub fn load_raw(
        &mut self,
        chunk: u64,
    ) -> Result<Option<(Vec<u8>, WadChunkCompression)>, WadError> {
        let Some(chunk) = self.chunks.get(&chunk) else {
            return Ok(None);
        };
        let mut data = vec![0u8; chunk.compressed_size() as usize];
        self.source
            .seek(SeekFrom::Start(chunk.data_offset() as u64))?;
        self.source.read_exact(&mut data)?;
        Ok(Some((data, chunk.compression_type())))
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

        let mut ecdsa_signature = None;
        let mut data_checksum = None;

        if major == 2 {
            let _ecdsa_length = reader.seek(SeekFrom::Current(1))?;
            ecdsa_signature.replace({
                let mut sig = vec![0u8; 81];
                reader.read_exact(&mut sig)?;
                sig
            });
            data_checksum.replace(reader.read_u64::<LE>()?);
        } else if major == 3 {
            ecdsa_signature.replace({
                let mut sig = vec![0u8; 256];
                reader.read_exact(&mut sig)?;
                sig
            });
            data_checksum.replace(reader.read_u64::<LE>()?);
        }

        if major == 1 || major == 2 {
            let _toc_start_offset = reader.seek(SeekFrom::Current(2))?;
            let _toc_chunk_size = reader.seek(SeekFrom::Current(2))?;
        }

        let chunk_count = reader.read_i32::<LE>()? as usize;
        let mut chunks = HashMap::<u64, WadChunk>::with_capacity(chunk_count);
        for _ in 0..chunk_count {
            let chunk = match (major, minor) {
                (3, 1) => WadChunk::read_v3_1(&mut reader),
                (3, 3) => WadChunk::read_v3_1(&mut reader), // 3_3 == 3_1 in this context
                (3, 4) => WadChunk::read_v3_4(&mut reader),
                _ => Err(WadError::InvalidVersion { major, minor }),
            }?;

            chunks
                .insert(chunk.path_hash(), chunk)
                .map_or(Ok(()), |chunk| {
                    Err(WadError::DuplicateChunk {
                        path_hash: chunk.path_hash(),
                    })
                })?;
        }

        Ok(Wad {
            ecdsa_signature,
            data_checksum,
            chunks,
            source,
        })
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
