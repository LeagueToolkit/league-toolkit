use crate::Uncompressed;
use std::io::Write;

impl Uncompressed {
    pub fn to_writer<W: Write + ?Sized>(&self, writer: &mut W) -> crate::Result<()> {
        unimplemented!("TODO: animation::asset::Uncompressed writing");
    }
}
