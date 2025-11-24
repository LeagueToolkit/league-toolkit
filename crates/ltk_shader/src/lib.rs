pub mod defines;
pub mod loader;
pub mod toc;

use byteorder::{ReadBytesExt, LE};
use std::io::{self, Read};

pub(crate) fn read_sized_string<R: Read>(reader: &mut R) -> io::Result<String> {
    let len = reader.read_u32::<LE>()?;
    let mut buf = vec![0u8; len as usize];
    reader.read_exact(&mut buf)?;
    String::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
