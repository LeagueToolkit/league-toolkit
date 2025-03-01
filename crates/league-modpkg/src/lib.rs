use binrw::{binrw, BinRead};
use byteorder::{ReadBytesExt, LE};
use chunk::ModpkgChunk;
use error::ModpkgError;
use io_ext::ReaderExt;
use std::{
    collections::HashMap,
    fmt::Display,
    io::{BufReader, Read, Seek, SeekFrom},
};
use xxhash_rust::{xxh3::xxh3_64, xxh64::xxh64};

pub mod builder;
mod chunk;
mod error;
mod license;
mod metadata;
pub mod utils;

pub use license::*;
pub use metadata::*;
pub use utils::*;

#[derive(Debug, PartialEq)]
pub struct Modpkg<TSource: Read + Seek> {
    signature: Vec<u8>,

    pub layers: HashMap<u64, ModpkgLayer>,
    pub chunk_paths: HashMap<u64, String>,

    pub metadata: ModpkgMetadata,

    /// The chunks in the mod package.
    ///
    /// The key is a tuple of the path hash and the layer hash.
    pub chunks: HashMap<(u64, u64), ModpkgChunk>,

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

impl<TSource: Read + Seek> Modpkg<TSource> {
    const MAGIC: [u8; 8] = *b"_modpkg_";

    pub fn mount_from_reader(mut source: TSource) -> Result<Self, ModpkgError> {
        let mut reader = BufReader::new(&mut source);

        let magic = reader.read_u64::<LE>()?;
        if magic != u64::from_le_bytes(Self::MAGIC) {
            return Err(ModpkgError::InvalidMagic(magic));
        }

        let version = reader.read_u32::<LE>()?;
        if version != 1 {
            return Err(ModpkgError::InvalidVersion(version));
        }

        let signature_size = reader.read_u32::<LE>()?;
        let chunk_count = reader.read_u32::<LE>()?;

        let mut signature = vec![0; signature_size as usize];
        reader.read_exact(&mut signature)?;

        let layer_count = reader.read_u32::<LE>()?;
        let mut layers = HashMap::new();
        for _ in 0..layer_count {
            let layer = ModpkgLayer::read(&mut reader)?;
            layers.insert(xxh3_64(layer.name.as_bytes()), layer);
        }

        let chunk_paths_count = reader.read_u32::<LE>()?;
        let mut chunk_paths = HashMap::new();
        for _ in 0..chunk_paths_count {
            let chunk_path = reader.read_str_until_nul()?;
            let chunk_path_hash = xxh64(chunk_path.as_bytes(), 0);
            chunk_paths.insert(chunk_path_hash, chunk_path);
        }

        let metadata = ModpkgMetadata::read(&mut reader)?;

        // Skip alignment
        let position = reader.stream_position()?;
        reader.seek(SeekFrom::Current((8 - (position % 8)) as i64))?;

        let mut chunks = HashMap::new();
        for _ in 0..chunk_count {
            let chunk = ModpkgChunk::read(&mut reader)?;
            chunks.insert((chunk.path_hash, chunk.layer_hash), chunk);
        }

        Ok(Self {
            signature,
            layers,
            chunk_paths,
            metadata,
            chunks,
            source,
        })
    }
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
