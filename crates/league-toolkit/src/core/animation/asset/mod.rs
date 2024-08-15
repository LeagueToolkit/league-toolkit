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

#[derive(Clone, Debug)]
pub enum AnimationAsset {
    Uncompressed(Uncompressed),
    Compressed(Compressed),
}

pub enum AnimationAssetType {
    Uncompressed,
    Compressed,
    Unknown,
}

impl AnimationAsset {
    pub fn from_reader<R: Read + Seek + ?Sized>(reader: &mut R) -> error::Result<AnimationAsset> {
        use crate::util::ReaderExt as _;
        use byteorder::{ReadBytesExt as _, LE};
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
