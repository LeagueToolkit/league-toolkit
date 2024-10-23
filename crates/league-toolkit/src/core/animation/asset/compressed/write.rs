use crate::core::animation;
use crate::core::animation::Compressed;
use std::io::Write;

impl Compressed {
    pub fn to_writer<W: Write + ?Sized>(&self, writer: &mut W) -> animation::Result<()> {
        unimplemented!("TODO: animation::asset::Compressed writing");
    }
}
