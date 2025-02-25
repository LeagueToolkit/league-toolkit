//! Animation assets
mod compressed;

pub use compressed::*;

mod uncompressed;

pub use uncompressed::*;

pub mod error;
mod error_metric;

pub use error::*;

use error::AssetParseError::UnknownAssetType;
use std::io;
use std::io::{Read, Seek, SeekFrom};

/// Encapsulates a .anm file
#[derive(Clone, Debug)]
pub enum AnimationAsset {
    Uncompressed(Uncompressed),
    Compressed(Compressed),
}

/// The type of animation asset
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum AnimationAssetType {
    Uncompressed,
    Compressed,
    Unknown,
}

impl AnimationAsset {
    pub fn from_reader<R: Read + Seek + ?Sized>(reader: &mut R) -> error::Result<AnimationAsset> {
        let asset_type = Self::identify_from_reader(reader)?;
        reader.seek(SeekFrom::Start(0))?;
        match asset_type {
            AnimationAssetType::Uncompressed => {
                Uncompressed::from_reader(reader).map(Self::Uncompressed)
            }
            AnimationAssetType::Compressed => Compressed::from_reader(reader).map(Self::Compressed),
            AnimationAssetType::Unknown => Err(UnknownAssetType),
        }
    }

    /// Reads the animation magic (8 bytes), and identifies the animation asset type
    pub fn identify_from_reader<R: Read + ?Sized>(
        reader: &mut R,
    ) -> io::Result<AnimationAssetType> {
        let mut magic = [0_u8; 8];
        reader.read_exact(&mut magic)?;

        Ok(match &magic {
            b"r3d2anmd" => AnimationAssetType::Uncompressed,
            b"r3d2canm" => AnimationAssetType::Compressed,
            _ => AnimationAssetType::Unknown,
        })
    }
}
