use chunk::ModpkgChunk;
use error::ModpkgError;
use metadata::ModpkgMetadata;
use std::{collections::HashMap, fmt::Display, io};
mod chunk;
mod error;
mod license;
mod metadata;
mod read;
mod utils;

pub const METADATA_CHUNK_NAME: &str = "__metadata__";
pub const LAYERS_CHUNK_NAME: &str = "__layers__";
pub const WADS_CHUNK_NAME: &str = "__wads__";
pub const CHUNK_PATHS_CHUNK_NAME: &str = "__chunk_paths__";

pub const METADATA_CHUNK_HASH: u64 = 0xc3b02c1cbcdff91f;
pub const LAYERS_CHUNK_HASH: u64 = 0xe8f354f18f398ee1;
pub const WADS_CHUNK_HASH: u64 = 0x67c34d7d3d2900df;
pub const CHUNK_PATHS_CHUNK_HASH: u64 = 0xbe4dc608d6e153c0;

#[derive(Debug, PartialEq)]
pub struct Modpkg<TSource: io::Read + io::Seek> {
    metadata: ModpkgMetadata,
    chunk_paths: Vec<String>,
    wad_paths: Vec<String>,
    chunks: HashMap<u64, ModpkgChunk>,

    source: TSource,
}

impl<TSource: io::Read + io::Seek> Modpkg<TSource> {
    pub fn metadata(&self) -> &ModpkgMetadata {
        &self.metadata
    }
    pub fn chunks(&self) -> &HashMap<u64, ModpkgChunk> {
        &self.chunks
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum ModpkgCompression {
    None = 0,
    Zstd = 1,
}

impl Display for ModpkgCompression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}",
            match self {
                ModpkgCompression::None => "none",
                ModpkgCompression::Zstd => "zstd",
            }
        )
    }
}

impl TryFrom<u8> for ModpkgCompression {
    type Error = ModpkgError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => ModpkgCompression::None,
            1 => ModpkgCompression::Zstd,
            _ => return Err(ModpkgError::InvalidCompressionType(value)),
        })
    }
}
