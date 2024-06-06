use std::io::{Read, Seek, SeekFrom};
use super::{Joint, ParseError};

#[derive(Debug, Clone, PartialEq)]
pub struct RigResource {
    flags: u16,
    name: String,
    asset_name: String,
    joints: Vec<Joint>,
    /// Influence id's
    influences: Vec<i16>,
}

impl RigResource {
    /// The FNV hash of the format token string
    const FORMAT_TOKEN: u32 = 0x22FD4FC3;

    pub fn from_reader<R: Read + Seek + ?Sized>(reader: &mut R) -> super::Result<Self> {
        use crate::util::ReaderExt as _;
        use byteorder::{ReadBytesExt as _, LE};

        reader.seek(SeekFrom::Start(4))?;
        let format_token = reader.read_u32::<LE>()?;
        reader.rewind()?;
        match format_token == Self::FORMAT_TOKEN {
            true => Self::read(reader),
            false => Self::read_legacy(reader),
        }
    }

    fn read<R: Read + Seek + ?Sized>(reader: &mut R) -> super::Result<Self> {
        use crate::util::ReaderExt as _;
        use byteorder::{ReadBytesExt as _, LE};

        let file_size = reader.read_u32::<LE>()?;
        let format_token = reader.read_u32::<LE>()?;
        let version = reader.read_u32::<LE>()?;
        if version != 0 {
            return Err(ParseError::InvalidFileVersion(version));
        }

        let flags = reader.read_u16::<LE>()?;
        let joint_count = reader.read_u16::<LE>()? as usize;
        let influences_count = reader.read_u32::<LE>()? as usize;
        let joints_off = reader.read_i32::<LE>()?;
        let joint_indices_off = reader.read_i32::<LE>()?;
        let influences_off = reader.read_i32::<LE>()?;
        let name_off = reader.read_i32::<LE>()?;
        let asset_name_off = reader.read_i32::<LE>()?;
        let bone_names_off = reader.read_i32::<LE>()?;

        // extension offsets
        for _ in 0..5 {
            reader.read_i32::<LE>()?;
        }


        let mut joints = Vec::with_capacity(joint_count);
        if joints_off > 0 {
            reader.seek(SeekFrom::Start(joints_off as u64))?;
            for _ in 0..joint_count {
                joints.push(Joint::from_reader(reader)?);
            }
        }

        let mut influences = Vec::with_capacity(influences_count);
        if influences_off > 0 {
            reader.seek(SeekFrom::Start(influences_off as u64))?;
            for _ in 0..influences_count {
                influences.push(reader.read_i16::<LE>()?);
            }
        }

        // These are sorted by hash in ascending order
        let mut joint_hash_ids = Vec::with_capacity(joint_count);
        if joint_indices_off > 0 {
            reader.seek(SeekFrom::Start(joint_indices_off as u64))?;
            for _ in 0..joint_count {
                let id = reader.read_i16::<LE>()?;
                reader.read_i16::<LE>()?;
                let hash = reader.read_u32::<LE>()?;
                joint_hash_ids.push((id, hash));
            }
        }

        let name = match name_off > 0 {
            true => {
                reader.seek(SeekFrom::Start(name_off as u64))?;
                reader.read_str_until_nul()?
            }
            false => String::new(),
        };

        let asset_name = match asset_name_off > 0 {
            true => {
                reader.seek(SeekFrom::Start(asset_name_off as u64))?;
                reader.read_str_until_nul()?
            }
            false => String::new()
        };


        Ok(Self {
            flags,
            name,
            asset_name,
            joints,
            influences,
        })
    }

    fn read_legacy<R: Read + ?Sized>(reader: &mut R) -> super::Result<Self> {
        use crate::util::ReaderExt as _;
        use byteorder::{ReadBytesExt as _, LE};
        unimplemented!("TODO: impl legacy skeleton");
    }
    pub fn flags(&self) -> u16 {
        self.flags
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn asset_name(&self) -> &str {
        &self.asset_name
    }
    pub fn joints(&self) -> &Vec<Joint> {
        &self.joints
    }
    pub fn influences(&self) -> &Vec<i16> {
        &self.influences
    }
}