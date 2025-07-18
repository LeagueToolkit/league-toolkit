use std::io::{BufReader, Read};

use byteorder::{ReadBytesExt as _, LE};
use num_enum::TryFromPrimitive as _;

use crate::WadError;

use super::{WadChunk, WadChunkCompression};

impl WadChunk {
    pub fn read_v3_1<R: Read>(reader: &mut BufReader<R>) -> Result<WadChunk, WadError> {
        let path_hash = reader.read_u64::<LE>()?;
        let data_offset = reader.read_u32::<LE>()? as usize;
        let compressed_size = reader.read_i32::<LE>()? as usize;
        let uncompressed_size = reader.read_i32::<LE>()? as usize;

        let type_frame_count = reader.read_u8()?;
        let frame_count = type_frame_count >> 4;
        let compression_type = WadChunkCompression::try_from_primitive(type_frame_count & 0xF)
            .map_err(|_| WadError::InvalidChunkCompression {
                compression: type_frame_count & 0xF,
            })?;

        let is_duplicated = reader.read_u8()? == 1;
        let start_frame = reader.read_u16::<LE>()?;
        let checksum = reader.read_u64::<LE>()?;

        Ok(WadChunk {
            path_hash,
            data_offset,
            compressed_size,
            uncompressed_size,
            compression_type,
            is_duplicated,
            frame_count,
            start_frame: start_frame as u32,
            checksum,
        })
    }

    pub fn read_v3_4<R: Read>(reader: &mut BufReader<R>) -> Result<WadChunk, WadError> {
        let path_hash = reader.read_u64::<LE>()?;
        let data_offset = reader.read_u32::<LE>()? as usize;
        let compressed_size = reader.read_u32::<LE>()? as usize;
        let uncompressed_size = reader.read_u32::<LE>()? as usize;

        let type_frame_count = reader.read_u8()?;
        let frame_count = type_frame_count >> 4;
        let compression_type = WadChunkCompression::try_from_primitive(type_frame_count & 0xF)
            .map_err(|_| WadError::InvalidChunkCompression {
                compression: type_frame_count & 0xF,
            })?;

        let start_frame = read_24_bit_subchunk_start_frame(reader)?;

        let checksum = reader.read_u64::<LE>()?;

        Ok(WadChunk {
            path_hash,
            data_offset,
            compressed_size,
            uncompressed_size,
            compression_type,
            is_duplicated: false, // v3.4 always has is_duplicated = false
            frame_count,
            start_frame: start_frame as u32,
            checksum,
        })
    }
}

pub(crate) fn read_24_bit_subchunk_start_frame<R: Read>(
    reader: &mut BufReader<R>,
) -> Result<u32, WadError> {
    let start_frame_hi = reader.read_u8()? as u32;
    let start_frame_lo = reader.read_u8()? as u32;
    let start_frame_mi = reader.read_u8()? as u32;
    let start_frame = start_frame_hi << 16 | start_frame_mi << 8 | start_frame_lo;

    Ok(start_frame)
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor};

    use super::read_24_bit_subchunk_start_frame;

    #[test]
    fn test_read_24_bit_subchunk_start_frame() {
        let mut reader = BufReader::new(Cursor::new([0x01, 0x03, 0x02]));
        let start_frame = read_24_bit_subchunk_start_frame(&mut reader).unwrap();
        assert_eq!(start_frame, 0x010203);
    }
}
