use crate::core::animation::{asset, Uncompressed};
use std::io::Read;

impl Uncompressed {
    pub fn from_reader<R: Read + ?Sized>(reader: &mut R) -> asset::Result<Self> {
        unimplemented!("TODO: animation::asset::Uncompressed reading");
    }
}
