use std::io::{Write};
use crate::core::animation;
use crate::core::animation::Uncompressed;

impl Uncompressed {
    pub fn to_writer<W: Write + ?Sized>(&self, writer: &mut W) -> animation::Result<()> {
        use byteorder::{LE, WriteBytesExt as _};
        use crate::util::WriterExt as _;
        unimplemented!("TODO: animation::asset::Uncompressed writing");
    }
}