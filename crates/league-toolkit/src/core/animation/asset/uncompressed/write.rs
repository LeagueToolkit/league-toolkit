use crate::core::animation;
use crate::core::animation::Uncompressed;
use std::io::Write;

impl Uncompressed {
    pub fn to_writer<W: Write + ?Sized>(&self, writer: &mut W) -> animation::Result<()> {
        unimplemented!("TODO: animation::asset::Uncompressed writing");
    }
}
