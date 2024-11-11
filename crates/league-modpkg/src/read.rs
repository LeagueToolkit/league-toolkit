use crate::{error::ModpkgError, Modpkg, ModpkgChunk, ModpkgMetadata};
use byteorder::{ReadBytesExt as _, LE};
use std::{
    collections::{hash_map::Entry, HashMap},
    io::{BufReader, Read, Seek, SeekFrom},
};

impl Modpkg {
    pub const MAGIC: u64 = u64::from_le_bytes(*b"_modpkg_");

    pub fn read(reader: &mut BufReader<impl Read + Seek>) -> Result<Self, ModpkgError> {
        let magic = reader.read_u64::<LE>()?;
        if magic != Self::MAGIC {
            return Err(ModpkgError::InvalidMagic(magic));
        }

        let version = reader.read_u32::<LE>()?;
        if version != 1 {
            return Err(ModpkgError::InvalidVersion(version));
        }

        let header_size = reader.read_u32::<LE>()?;
        let metadata_size = reader.read_u32::<LE>()? as usize;
        let signature_size = reader.read_u32::<LE>()? as usize;
        let chunk_paths_size = reader.read_u32::<LE>()? as usize;
        let wad_paths_size = reader.read_u32::<LE>()? as usize;
        let layers_size = reader.read_u32::<LE>()? as usize;
        let chunk_count = reader.read_u32::<LE>()?;

        let current_pos = reader.seek(SeekFrom::Current(0))?;
        if header_size != current_pos as u32 {
            return Err(ModpkgError::InvalidHeaderSize {
                header_size,
                actual_size: current_pos,
            });
        }

        let mut metadata = Vec::with_capacity(metadata_size);
        let mut signature = Vec::with_capacity(signature_size);
        let mut chunk_paths = Vec::with_capacity(chunk_paths_size);
        let mut wad_paths = Vec::with_capacity(wad_paths_size);
        let mut layers = Vec::with_capacity(layers_size);

        reader.read_exact(&mut metadata)?;
        reader.read_exact(&mut signature)?;
        reader.read_exact(&mut chunk_paths)?;
        reader.read_exact(&mut wad_paths)?;
        reader.read_exact(&mut layers)?;

        let metadata: ModpkgMetadata = rmp_serde::from_slice(&metadata)?;
        let chunk_paths: Vec<String> = rmp_serde::from_slice(&chunk_paths)?;
        let wad_paths: Vec<String> = rmp_serde::from_slice(&wad_paths)?;

        let layers: Vec<String> = rmp_serde::from_slice(&layers)?;
        if !layers.contains(&"base".to_string()) {
            return Err(ModpkgError::MissingBaseLayer);
        }

        let chunks = Self::read_chunks(reader, chunk_count)?;

        Ok(Self {
            metadata,
            chunk_paths,
            wad_paths,
            chunks,
        })
    }

    fn read_chunks(
        reader: &mut BufReader<impl Read>,
        chunk_count: u32,
    ) -> Result<HashMap<u64, ModpkgChunk>, ModpkgError> {
        let mut chunks = HashMap::with_capacity(chunk_count as usize);
        let mut last_hash = 0;

        for _ in 0..chunk_count {
            let chunk = ModpkgChunk::read(reader)?;
            let current_hash = chunk.path_hash();

            if current_hash <= last_hash && last_hash != 0 {
                return Err(ModpkgError::UnsortedChunks {
                    previous: last_hash,
                    current: current_hash,
                });
            }

            match chunks.entry(current_hash) {
                Entry::Occupied(_) => {
                    return Err(ModpkgError::DuplicateChunk(current_hash));
                }
                Entry::Vacant(entry) => {
                    last_hash = current_hash;
                    entry.insert(chunk);
                }
            }
        }

        Ok(chunks)
    }
}
