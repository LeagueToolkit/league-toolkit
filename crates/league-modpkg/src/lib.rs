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
use xxhash_rust::{xxh3::xxh3_64, xxh64::xxh64};

pub mod builder;
mod chunk;
mod error;
mod license;
mod metadata;
pub mod utils;

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
    #[bw(calc = layers.len() as u32)]
    layer_count: u32,
    #[br(count = layer_count, map = |m: Vec<ModpkgLayer>| m.into_iter().map(|c| (xxh3_64(c.name.as_bytes()), c)).collect())]
    #[bw(map = |m| m.values().cloned().collect_vec())]
    pub layers: HashMap<u64, ModpkgLayer>,

    #[br(temp)]
    #[bw(calc = chunk_paths.len() as u32)]
    chunk_path_count: u32,
    #[br(count = chunk_path_count, map = |m: Vec<NullString>| m.into_iter().map(|c| (xxh64(&c.0, 0), c)).collect())]
    #[bw(map = |m| m.values().cloned().collect_vec())]
    pub chunk_paths: HashMap<u64, NullString>,

    pub metadata: ModpkgMetadata,

    /// The chunks in the mod package.
    ///
    /// The key is a tuple of the path hash and the layer hash.
    // alan: pretty sure this works to align the individual chunks - https://github.com/jam1garner/binrw/issues/68
    #[brw(align_before = 8)]
    #[br(count = chunk_count, map = |m: Vec<ModpkgChunk>| {
        m.into_iter().map(|c| ((c.path_hash, c.layer_hash), c)).collect()
    })]
    #[bw(map = |m| m.values().copied().collect_vec())]
    pub chunks: HashMap<(u64, u64), ModpkgChunk>,

    #[brw(ignore)]
    /// The original byte source.
    source: TSource,
}

#[binrw]
#[brw(little)]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct ModpkgLayer {
    #[br(temp)]
    #[bw(calc = name.len() as u32)]
    name_len: u32,
    #[br(count = name_len, try_map = String::from_utf8)]
    #[bw(map = |s| s.as_bytes().to_vec())]
    pub name: String,

    pub priority: i32,
}

#[binrw]
#[brw(little, repr = u8)]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Default)]
pub enum ModpkgCompression {
    #[default]
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
