use std::io::Write;

use byteorder::{WriteBytesExt as _, LE};

use crate::WadError;

use super::WadChunk;

impl WadChunk {
    pub fn write_v3_4<W: Write>(&self, writer: &mut W) -> Result<(), WadError> {
        writer.write_u64::<LE>(self.path_hash)?;
        writer.write_u32::<LE>(self.data_offset as u32)?;
        writer.write_u32::<LE>(self.compressed_size as u32)?;
        writer.write_u32::<LE>(self.uncompressed_size as u32)?;

        let type_frame_count = (self.frame_count << 4) | (self.compression_type as u8 & 0xF);
        writer.write_u8(type_frame_count)?;

        write_24_bit_subchunk_start_frame(writer, self.start_frame)?;

        writer.write_u64::<LE>(self.checksum)?;

        Ok(())
    }
}

pub(crate) fn write_24_bit_subchunk_start_frame<W: Write>(
    writer: &mut W,
    start_frame: u32,
) -> Result<(), WadError> {
    writer.write_u8(((start_frame >> 16) & 0xFF) as u8)?;
    writer.write_u8((start_frame & 0xFF) as u8)?;
    writer.write_u8(((start_frame >> 8) & 0xFF) as u8)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::write_24_bit_subchunk_start_frame;

    #[test]
    fn test_write_24_bit_subchunk_start_frame() {
        let mut writer = Vec::new();
        write_24_bit_subchunk_start_frame(&mut writer, 0x010302).unwrap();
        assert_eq!(writer, [0x01, 0x02, 0x03]);
    }
}
