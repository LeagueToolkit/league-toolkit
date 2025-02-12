use byteorder::{ReadBytesExt as _, LE};
use io_ext::ReaderExt as _;
use std::{
    collections::{hash_map::Entry, HashMap},
    io::{BufReader, Read},
};

use crate::{error::ModpkgError, Modpkg, ModpkgAuthor, ModpkgChunk, ModpkgLicense};

impl Modpkg {
    pub const MAGIC: u64 = u64::from_le_bytes(*b"_modpkg_");

    pub fn read(reader: &mut BufReader<impl Read>) -> Result<Self, ModpkgError> {
        let magic = reader.read_u64::<LE>()?;
        if magic != Self::MAGIC {
            return Err(ModpkgError::InvalidMagic(magic));
        }

        let version = reader.read_u32::<LE>()?;
        if version != 1 {
            return Err(ModpkgError::InvalidVersion(version));
        }

        let name = reader.read_len_prefixed_string::<LE>()?;
        let display_name = reader.read_len_prefixed_string::<LE>()?;
        let description = reader.read_len_prefixed_string::<LE>()?;
        let version = reader.read_len_prefixed_string::<LE>()?;
        let distributor = reader.read_len_prefixed_string::<LE>()?;

        let authors = Self::read_authors(reader)?;
        let license = ModpkgLicense::read(reader)?;
        let chunks = Self::read_chunks(reader)?;
        Ok(Self {
            name,
            display_name,
            description: match description.len() {
                0 => None,
                _ => Some(description),
            },
            version,
            distributor: match distributor.len() {
                0 => None,
                _ => Some(distributor),
            },
            authors,
            license,
            chunks,
        })
    }

    fn read_authors(reader: &mut BufReader<impl Read>) -> Result<Vec<ModpkgAuthor>, ModpkgError> {
        let count = reader.read_u32::<LE>()?;
        let mut authors = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let name = reader.read_len_prefixed_string::<LE>()?;
            let role = reader.read_len_prefixed_string::<LE>()?;

            authors.push(ModpkgAuthor {
                name,
                role: match role.len() {
                    0 => None,
                    _ => Some(role),
                },
            });
        }

        Ok(authors)
    }

    fn read_chunks(
        reader: &mut BufReader<impl Read>,
    ) -> Result<HashMap<u64, ModpkgChunk>, ModpkgError> {
        let chunk_count = reader.read_u32::<LE>()?;
        let mut chunks = HashMap::with_capacity(chunk_count as usize);
        for _ in 0..chunk_count {
            let chunk = ModpkgChunk::read(reader)?;
            match chunks.entry(chunk.path_hash()) {
                Entry::Occupied(_) => {
                    return Err(ModpkgError::DuplicateChunk(chunk.path_hash()));
                }
                Entry::Vacant(entry) => {
                    entry.insert(chunk);
                }
            }
        }

        Ok(chunks)
    }
}
