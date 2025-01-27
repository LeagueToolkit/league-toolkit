use crate::{
    error::ModpkgError, metadata::ModpkgMetadata, Modpkg, ModpkgChunk, ModpkgCompression,
    CHUNK_PATHS_CHUNK_HASH, METADATA_CHUNK_HASH, WADS_CHUNK_HASH,
};
use byteorder::{ReadBytesExt as _, LE};
use io_ext::{measure, window, ReaderExt as _};
use std::{
    collections::{hash_map::Entry, HashMap},
    io::{BufReader, Read, Seek},
};

impl<TSource: Read + Seek> Modpkg<TSource> {
    pub const MAGIC: u64 = u64::from_le_bytes(*b"_modpkg_");

    pub fn mount(mut source: TSource) -> Result<Self, ModpkgError> {
        let mut reader = BufReader::new(&mut source);

        let (real_header_size, (header_size, signature_size, chunk_count)) =
            measure(&mut reader, |reader| {
                let magic = reader.read_u64::<LE>()?;
                if magic != Self::MAGIC {
                    return Err(ModpkgError::InvalidMagic(magic));
                }

                let version = reader.read_u32::<LE>()?;
                if version != 1 {
                    return Err(ModpkgError::InvalidVersion(version));
                }

                let header_size = reader.read_u32::<LE>()?;
                let signature_size = reader.read_u32::<LE>()? as usize;
                let chunk_count = reader.read_u32::<LE>()?;

                Ok((header_size, signature_size, chunk_count))
            })?;

        if header_size != real_header_size as u32 {
            return Err(ModpkgError::InvalidHeaderSize {
                header_size,
                actual_size: real_header_size,
            });
        }

        let mut signature = vec![0; signature_size];
        reader.read_exact(&mut signature)?;

        let chunks = Self::read_chunks(&mut reader, chunk_count)?;

        let metadata = Self::read_metadata_chunk(&mut reader, &chunks)?;
        let chunk_paths = Self::read_chunk_paths_chunk(&mut reader, &chunks)?;
        let wad_paths = Self::read_wad_paths_chunk(&mut reader, &chunks)?;

        Ok(Self {
            metadata,
            chunk_paths,
            wad_paths,
            chunks,
            source,
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

    fn read_metadata_chunk(
        reader: &mut BufReader<impl Read + Seek>,
        chunks: &HashMap<u64, ModpkgChunk>,
    ) -> Result<ModpkgMetadata, ModpkgError> {
        ModpkgMetadata::read(&mut BufReader::new(
            Self::read_auxiliary_chunk(
                METADATA_CHUNK_HASH,
                ModpkgCompression::None,
                reader,
                chunks,
            )?
            .as_slice(),
        ))
    }

    fn read_chunk_paths_chunk(
        reader: &mut BufReader<impl Read + Seek>,
        chunks: &HashMap<u64, ModpkgChunk>,
    ) -> Result<Vec<String>, ModpkgError> {
        Self::read_string_table(&Self::read_auxiliary_chunk(
            CHUNK_PATHS_CHUNK_HASH,
            ModpkgCompression::None,
            reader,
            chunks,
        )?)
    }

    fn read_wad_paths_chunk(
        reader: &mut BufReader<impl Read + Seek>,
        chunks: &HashMap<u64, ModpkgChunk>,
    ) -> Result<Vec<String>, ModpkgError> {
        Self::read_string_table(&Self::read_auxiliary_chunk(
            WADS_CHUNK_HASH,
            ModpkgCompression::None,
            reader,
            chunks,
        )?)
    }

    fn read_auxiliary_chunk(
        path_hash: u64,
        expected_compression: ModpkgCompression,
        reader: &mut BufReader<impl Read + Seek>,
        chunks: &HashMap<u64, ModpkgChunk>,
    ) -> Result<Vec<u8>, ModpkgError> {
        let chunk = chunks
            .get(&path_hash)
            .ok_or(ModpkgError::MissingChunk(path_hash))?;

        if chunk.compression() != expected_compression {
            return Err(ModpkgError::UnexpectedCompressionType {
                chunk: path_hash,
                expected: expected_compression,
                actual: chunk.compression(),
            });
        }

        let mut data = vec![0; chunk.uncompressed_size()];
        window(reader, chunk.data_offset() as u64, |reader| {
            reader.read_exact(&mut data)
        })?;

        Ok(data)
    }

    fn read_string_table(data: &[u8]) -> Result<Vec<String>, ModpkgError> {
        let mut reader = BufReader::new(data);

        let count = reader.read_u32::<LE>()?;
        let mut strings = Vec::with_capacity(count as usize);

        for _ in 0..count {
            strings.push(reader.read_len_prefixed_string::<LE>()?);
        }

        Ok(strings)
    }
}
