use std::io::Read;
use crate::core::animation::{asset, Uncompressed};

impl Uncompressed {
    pub fn from_reader<R: Read + ?Sized>(reader: &mut R) -> asset::Result<Self> {
        use crate::util::ReaderExt as _;
        use byteorder::{ReadBytesExt as _, LE};
        unimplemented!("TODO: animation::asset::Uncompressed reading");
    }
}