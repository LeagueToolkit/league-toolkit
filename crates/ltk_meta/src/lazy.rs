use std::io::{self, Read, Seek};

use byteorder::{ReadBytesExt as _, LE};
use ltk_hash::{BinHash, ReadBytesExt as _};

use crate::{
    header::{Header, PatchHeader, PropHeader, ReadHeaderError},
    traits::ReaderExt,
    PropertyValueEnum,
};

#[derive(Debug)]
pub struct LazyBin<'r, R: Read + Seek + ?Sized> {
    pub header: PropHeader,
    pub patch: Option<PatchHeader>,
    reader: &'r mut R,
}

#[derive(Debug)]
pub struct LazyObject<'r, R: Read + Seek + ?Sized> {
    pub header: PropHeader,
    reader: &'r mut R,

    pub size: u32,
    pub prop_count: u16,
}

#[derive(Debug, thiserror::Error)]
pub enum ReadErr {
    #[error(transparent)]
    Error(#[from] crate::Error),
    #[error(transparent)]
    ReadHeader(#[from] ReadHeaderError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl<'r, R: Read + Seek + ?Sized> LazyBin<'r, R> {
    pub fn from_reader(reader: &'r mut R) -> Result<Self, ReadErr> {
        let (header, patch) = Header::from_reader(reader)?.into_parts();
        Ok(Self {
            header,
            patch,
            reader,
        })
    }

    pub fn object(self, path: impl Into<BinHash>) -> Result<Option<LazyObject<'r, R>>, ReadErr> {
        let path = path.into();

        self.reader.seek_relative(
            i64::from(self.header.object_count) * i64::try_from(size_of::<BinHash>()).unwrap(),
        )?;

        for _ in 0..self.header.object_count {
            let size = self.reader.read_u32::<LE>()?;
            let cur_path = self.reader.read_bin_hash::<LE>()?;
            if cur_path == path {
                return Ok(Some(LazyObject {
                    prop_count: self.reader.read_u16::<LE>()?,
                    header: self.header,
                    reader: self.reader,
                    size,
                }));
            }
            self.reader.seek_relative(i64::from(size) - 4)?;
        }

        Ok(None)
    }
}
impl<'r, R: Read + Seek + ?Sized> LazyObject<'r, R> {
    pub fn properties<'a>(
        &'a mut self,
        legacy: bool,
    ) -> impl Iterator<Item = Result<(BinHash, PropertyValueEnum), ReadErr>> + use<'r, 'a, R> {
        (0..self.prop_count).map(move |_| {
            let name = self.reader.read_bin_hash::<LE>()?;
            let kind = self.reader.read_property_kind(legacy)?;
            let value = kind.read(self.reader, legacy)?;
            Ok((name, value))
        })
    }
}

#[derive(Debug)]
pub struct LazyField<'r, R: Read + Seek + ?Sized> {
    pub header: PropHeader,
    pub reader: &'r mut R,

    pub size: u32,
    pub prop_count: u16,
}
