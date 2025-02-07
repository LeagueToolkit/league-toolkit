use binrw::{binrw, NullString};
use chunk::ModpkgChunk;
use error::ModpkgError;
use itertools::Itertools;
use metadata::ModpkgMetadata;
use std::{
    collections::HashMap,
    fmt::Display,
    io::{Read, Seek},
};
mod chunk;
mod error;
mod license;
mod metadata;
mod utils;

#[binrw]
#[brw(little, magic = b"_modpkg_")]
#[derive(Debug, PartialEq)]
pub struct Modpkg<TSource: Read + Seek + Default> {
    #[br(temp, assert(version == 1))]
    #[bw(calc = 1)]
    version: u32,
    #[br(temp)]
    #[bw(calc = signature.len() as u32)]
    signature_size: u32,
    #[br(temp)]
    #[bw(calc = chunks.len() as u32)]
    chunk_count: u32,

    #[br(count = signature_size)]
    signature: Vec<u8>,

    #[br(temp)]
    #[bw(calc = chunk_paths.len() as u32)]
    chunk_path_count: u32,
    #[br(count = chunk_path_count)]
    chunk_paths: Vec<NullString>,

    #[br(temp)]
    #[bw(calc = wad_paths.len() as u32)]
    wad_path_count: u32,
    #[br(count = wad_path_count)]
    wad_paths: Vec<NullString>,

    #[br(count = chunk_count, map = |m: Vec<ModpkgChunk>| m.into_iter().map(|c| (c.path_hash, c)).collect())]
    #[bw(map = |m| m.values().copied().collect_vec())]
    chunks: HashMap<u64, ModpkgChunk>,

    metadata: ModpkgMetadata,

    #[brw(ignore)]
    /// The original byte source.
    source: TSource,
}

impl<TSource: Read + Seek + Default> Modpkg<TSource> {
    pub fn metadata(&self) -> &ModpkgMetadata {
        &self.metadata
    }
    pub fn chunks(&self) -> &HashMap<u64, ModpkgChunk> {
        &self.chunks
    }
}

#[binrw]
#[brw(little, repr = u8)]
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
