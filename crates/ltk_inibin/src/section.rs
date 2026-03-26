use std::io::{Read, Seek, SeekFrom, Write};

use byteorder::{LittleEndian as LE, ReadBytesExt, WriteBytesExt};
use glam::{Vec2, Vec3, Vec4};
use indexmap::IndexMap;

use crate::error::Result;
use crate::value::Value;
use crate::value_kind::ValueKind;

/// A typed bucket of key-value pairs within an inibin file.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    kind: ValueKind,
    properties: IndexMap<u32, Value>,
}

impl Section {
    pub(crate) fn new(kind: ValueKind) -> Self {
        Self {
            kind,
            properties: IndexMap::new(),
        }
    }

    // ── Accessors ──────────────────────────────────────────────────

    pub fn get(&self, key: u32) -> Option<&Value> {
        self.properties.get(&key)
    }

    pub fn insert(&mut self, key: u32, value: Value) {
        self.properties.insert(key, value);
    }

    pub fn remove(&mut self, key: u32) -> Option<Value> {
        self.properties.shift_remove(&key)
    }

    pub fn len(&self) -> usize {
        self.properties.len()
    }

    pub fn is_empty(&self) -> bool {
        self.properties.is_empty()
    }

    pub fn kind(&self) -> ValueKind {
        self.kind
    }

    pub fn iter(&self) -> impl Iterator<Item = (u32, &Value)> {
        self.properties.iter().map(|(&k, v)| (k, v))
    }

    // ── Reading ────────────────────────────────────────────────────

    pub(crate) fn read_non_string<R: Read>(reader: &mut R, kind: ValueKind) -> Result<Self> {
        let value_count = reader.read_u16::<LE>()? as usize;
        let hashes = read_hashes(reader, value_count)?;
        let mut properties = IndexMap::with_capacity(value_count);

        match kind {
            ValueKind::INT32_LIST => {
                for hash in hashes {
                    properties.insert(hash, Value::I32(reader.read_i32::<LE>()?));
                }
            }
            ValueKind::F32_LIST => {
                for hash in hashes {
                    properties.insert(hash, Value::F32(reader.read_f32::<LE>()?));
                }
            }
            ValueKind::U8_LIST => {
                for hash in hashes {
                    properties.insert(hash, Value::U8(reader.read_u8()?));
                }
            }
            ValueKind::INT16_LIST => {
                for hash in hashes {
                    properties.insert(hash, Value::I16(reader.read_i16::<LE>()?));
                }
            }
            ValueKind::INT8_LIST => {
                for hash in hashes {
                    properties.insert(hash, Value::I8(reader.read_u8()?));
                }
            }
            ValueKind::BIT_LIST => {
                // 8 booleans packed per byte, extracted LSB to MSB
                let mut current_byte: u8 = 0;
                for (i, hash) in hashes.into_iter().enumerate() {
                    if i % 8 == 0 {
                        current_byte = reader.read_u8()?;
                    }
                    let bit = (current_byte >> (i % 8)) & 1 != 0;
                    properties.insert(hash, Value::Bool(bit));
                }
            }
            ValueKind::VEC3_U8_LIST => {
                for hash in hashes {
                    let x = reader.read_u8()?;
                    let y = reader.read_u8()?;
                    let z = reader.read_u8()?;
                    properties.insert(hash, Value::Vec3U8([x, y, z]));
                }
            }
            ValueKind::VEC3_F32_LIST => {
                for hash in hashes {
                    let x = reader.read_f32::<LE>()?;
                    let y = reader.read_f32::<LE>()?;
                    let z = reader.read_f32::<LE>()?;
                    properties.insert(hash, Value::Vec3F32(Vec3::new(x, y, z)));
                }
            }
            ValueKind::VEC2_U8_LIST => {
                for hash in hashes {
                    let x = reader.read_u8()?;
                    let y = reader.read_u8()?;
                    properties.insert(hash, Value::Vec2U8([x, y]));
                }
            }
            ValueKind::VEC2_F32_LIST => {
                for hash in hashes {
                    let x = reader.read_f32::<LE>()?;
                    let y = reader.read_f32::<LE>()?;
                    properties.insert(hash, Value::Vec2F32(Vec2::new(x, y)));
                }
            }
            ValueKind::VEC4_U8_LIST => {
                for hash in hashes {
                    let x = reader.read_u8()?;
                    let y = reader.read_u8()?;
                    let z = reader.read_u8()?;
                    let w = reader.read_u8()?;
                    properties.insert(hash, Value::Vec4U8([x, y, z, w]));
                }
            }
            ValueKind::VEC4_F32_LIST => {
                for hash in hashes {
                    let x = reader.read_f32::<LE>()?;
                    let y = reader.read_f32::<LE>()?;
                    let z = reader.read_f32::<LE>()?;
                    let w = reader.read_f32::<LE>()?;
                    properties.insert(hash, Value::Vec4F32(Vec4::new(x, y, z, w)));
                }
            }
            ValueKind::INT64_LIST => {
                for hash in hashes {
                    properties.insert(hash, Value::I64(reader.read_i64::<LE>()?));
                }
            }
            _ => {}
        }

        Ok(Self { kind, properties })
    }

    pub(crate) fn read_string_list<R: Read + Seek>(
        reader: &mut R,
        string_data_offset: u64,
    ) -> Result<Self> {
        let value_count = reader.read_u16::<LE>()? as usize;
        let hashes = read_hashes(reader, value_count)?;

        let mut offsets = Vec::with_capacity(value_count);
        for _ in 0..value_count {
            offsets.push(reader.read_u16::<LE>()?);
        }

        // Seek to each string offset, read null-terminated, then seek back
        let mut properties = IndexMap::with_capacity(value_count);
        for (i, hash) in hashes.into_iter().enumerate() {
            let saved_pos = reader.stream_position()?;
            reader.seek(SeekFrom::Start(string_data_offset + offsets[i] as u64))?;
            let s = read_null_terminated_string(reader)?;
            reader.seek(SeekFrom::Start(saved_pos))?;
            properties.insert(hash, Value::String(s));
        }

        Ok(Self {
            kind: ValueKind::STRING_LIST,
            properties,
        })
    }

    /// Version 1 (legacy): hashes are provided externally, not read from the set header.
    pub(crate) fn read_string_list_v1<R: Read + Seek>(
        reader: &mut R,
        hashes: Vec<u32>,
        string_data_offset: u64,
    ) -> Result<Self> {
        let value_count = hashes.len();

        let mut offsets = Vec::with_capacity(value_count);
        for _ in 0..value_count {
            offsets.push(reader.read_u16::<LE>()?);
        }

        let mut properties = IndexMap::with_capacity(value_count);
        for (i, hash) in hashes.into_iter().enumerate() {
            let saved_pos = reader.stream_position()?;
            reader.seek(SeekFrom::Start(string_data_offset + offsets[i] as u64))?;
            let s = read_null_terminated_string(reader)?;
            reader.seek(SeekFrom::Start(saved_pos))?;
            properties.insert(hash, Value::String(s));
        }

        Ok(Self {
            kind: ValueKind::STRING_LIST,
            properties,
        })
    }

    // ── Writing ────────────────────────────────────────────────────

    pub(crate) fn write_non_string<W: Write>(&self, writer: &mut W) -> Result<()> {
        let count = self.properties.len() as u16;
        writer.write_u16::<LE>(count)?;

        let mut entries: Vec<_> = self.properties.iter().collect();
        entries.sort_by_key(|(&k, _)| k);

        for (&hash, _) in &entries {
            writer.write_u32::<LE>(hash)?;
        }

        for (_, value) in &entries {
            match value {
                Value::I32(v) => writer.write_i32::<LE>(*v)?,
                Value::F32(v) => writer.write_f32::<LE>(*v)?,
                Value::U8(v) => writer.write_u8(*v)?,
                Value::I16(v) => writer.write_i16::<LE>(*v)?,
                Value::I8(v) => writer.write_u8(*v)?,
                Value::Bool(_) => unreachable!("Bool values are written via write_bit_list"),
                Value::Vec3U8([x, y, z]) => {
                    writer.write_u8(*x)?;
                    writer.write_u8(*y)?;
                    writer.write_u8(*z)?;
                }
                Value::Vec3F32(v) => {
                    writer.write_f32::<LE>(v.x)?;
                    writer.write_f32::<LE>(v.y)?;
                    writer.write_f32::<LE>(v.z)?;
                }
                Value::Vec2U8([x, y]) => {
                    writer.write_u8(*x)?;
                    writer.write_u8(*y)?;
                }
                Value::Vec2F32(v) => {
                    writer.write_f32::<LE>(v.x)?;
                    writer.write_f32::<LE>(v.y)?;
                }
                Value::Vec4U8([x, y, z, w]) => {
                    writer.write_u8(*x)?;
                    writer.write_u8(*y)?;
                    writer.write_u8(*z)?;
                    writer.write_u8(*w)?;
                }
                Value::Vec4F32(v) => {
                    writer.write_f32::<LE>(v.x)?;
                    writer.write_f32::<LE>(v.y)?;
                    writer.write_f32::<LE>(v.z)?;
                    writer.write_f32::<LE>(v.w)?;
                }
                Value::I64(v) => writer.write_i64::<LE>(*v)?,
                Value::String(_) => {}
            }
        }

        Ok(())
    }

    /// BitList uses special packing: 8 booleans per byte, LSB to MSB.
    pub(crate) fn write_bit_list<W: Write>(&self, writer: &mut W) -> Result<()> {
        let count = self.properties.len() as u16;
        writer.write_u16::<LE>(count)?;

        let mut entries: Vec<_> = self.properties.iter().collect();
        entries.sort_by_key(|(&k, _)| k);

        for (&hash, _) in &entries {
            writer.write_u32::<LE>(hash)?;
        }

        let mut current_byte: u8 = 0;
        for (i, (_, value)) in entries.iter().enumerate() {
            if let Value::Bool(v) = value {
                if *v {
                    current_byte |= 1 << (i % 8);
                }
            }
            if (i % 8 == 7) || (i == entries.len() - 1) {
                writer.write_u8(current_byte)?;
                current_byte = 0;
            }
        }

        Ok(())
    }

    /// Returns the string data bytes separately (written after the offset table).
    pub(crate) fn write_string_list<W: Write>(&self, writer: &mut W) -> Result<Vec<u8>> {
        let count = self.properties.len() as u16;
        writer.write_u16::<LE>(count)?;

        let mut entries: Vec<_> = self.properties.iter().collect();
        entries.sort_by_key(|(&k, _)| k);

        for (&hash, _) in &entries {
            writer.write_u32::<LE>(hash)?;
        }

        let mut string_data = Vec::new();
        let mut offsets = Vec::with_capacity(entries.len());

        for (_, value) in &entries {
            if let Value::String(s) = value {
                offsets.push(string_data.len() as u16);
                string_data.extend_from_slice(s.as_bytes());
                string_data.push(0);
            }
        }

        for offset in &offsets {
            writer.write_u16::<LE>(*offset)?;
        }

        Ok(string_data)
    }

    pub(crate) fn string_data_length(&self) -> u16 {
        let mut len: usize = 0;
        for value in self.properties.values() {
            if let Value::String(s) = value {
                len += s.len() + 1;
            }
        }
        len as u16
    }
}

// ── Helpers ────────────────────────────────────────────────────────

fn read_hashes<R: Read>(reader: &mut R, count: usize) -> Result<Vec<u32>> {
    let mut hashes = Vec::with_capacity(count);
    for _ in 0..count {
        hashes.push(reader.read_u32::<LE>()?);
    }
    Ok(hashes)
}

fn read_null_terminated_string<R: Read>(reader: &mut R) -> Result<String> {
    let mut bytes = Vec::new();
    loop {
        let byte = reader.read_u8()?;
        if byte == 0 {
            break;
        }
        bytes.push(byte);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Cursor;

    #[test]
    fn test_read_int32_list() {
        let mut data = Vec::new();
        data.extend_from_slice(&2u16.to_le_bytes());
        data.extend_from_slice(&0xAAAA0001u32.to_le_bytes());
        data.extend_from_slice(&0xAAAA0002u32.to_le_bytes());
        data.extend_from_slice(&42i32.to_le_bytes());
        data.extend_from_slice(&(-7i32).to_le_bytes());

        let mut cursor = Cursor::new(data);
        let set = Section::read_non_string(&mut cursor, ValueKind::INT32_LIST).unwrap();

        assert_eq!(set.len(), 2);
        assert_eq!(set.get(0xAAAA0001), Some(&Value::I32(42)));
        assert_eq!(set.get(0xAAAA0002), Some(&Value::I32(-7)));
    }

    #[test]
    fn test_read_float32_list() {
        let mut data = Vec::new();
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&0xBBBB0001u32.to_le_bytes());
        data.extend_from_slice(&3.125f32.to_le_bytes());

        let mut cursor = Cursor::new(data);
        let set = Section::read_non_string(&mut cursor, ValueKind::F32_LIST).unwrap();

        assert_eq!(set.len(), 1);
        if let Some(Value::F32(v)) = set.get(0xBBBB0001) {
            approx::assert_relative_eq!(*v, 3.125);
        } else {
            panic!("Expected F32");
        }
    }

    #[test]
    fn test_read_u8_list() {
        let mut data = Vec::new();
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&0xCCCC0001u32.to_le_bytes());
        data.push(100u8);

        let mut cursor = Cursor::new(data);
        let set = Section::read_non_string(&mut cursor, ValueKind::U8_LIST).unwrap();

        assert_eq!(set.get(0xCCCC0001), Some(&Value::U8(100)));
    }

    #[test]
    fn test_read_int16_list() {
        let mut data = Vec::new();
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&0xDDDD0001u32.to_le_bytes());
        data.extend_from_slice(&(-123i16).to_le_bytes());

        let mut cursor = Cursor::new(data);
        let set = Section::read_non_string(&mut cursor, ValueKind::INT16_LIST).unwrap();

        assert_eq!(set.get(0xDDDD0001), Some(&Value::I16(-123)));
    }

    #[test]
    fn test_read_int8_list() {
        let mut data = Vec::new();
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&0xEEEE0001u32.to_le_bytes());
        data.push(200u8);

        let mut cursor = Cursor::new(data);
        let set = Section::read_non_string(&mut cursor, ValueKind::INT8_LIST).unwrap();

        assert_eq!(set.get(0xEEEE0001), Some(&Value::I8(200)));
    }

    #[test]
    fn test_read_bit_list() {
        let mut data = Vec::new();
        data.extend_from_slice(&3u16.to_le_bytes());
        data.extend_from_slice(&0x00000001u32.to_le_bytes());
        data.extend_from_slice(&0x00000002u32.to_le_bytes());
        data.extend_from_slice(&0x00000003u32.to_le_bytes());
        data.push(0b00000101u8); // bits: 1,0,1

        let mut cursor = Cursor::new(data);
        let set = Section::read_non_string(&mut cursor, ValueKind::BIT_LIST).unwrap();

        assert_eq!(set.get(0x00000001), Some(&Value::Bool(true)));
        assert_eq!(set.get(0x00000002), Some(&Value::Bool(false)));
        assert_eq!(set.get(0x00000003), Some(&Value::Bool(true)));
    }

    #[test]
    fn test_read_vec3_f32_list() {
        let mut data = Vec::new();
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&0x11110001u32.to_le_bytes());
        data.extend_from_slice(&1.0f32.to_le_bytes());
        data.extend_from_slice(&2.0f32.to_le_bytes());
        data.extend_from_slice(&3.0f32.to_le_bytes());

        let mut cursor = Cursor::new(data);
        let set = Section::read_non_string(&mut cursor, ValueKind::VEC3_F32_LIST).unwrap();

        assert_eq!(
            set.get(0x11110001),
            Some(&Value::Vec3F32(Vec3::new(1.0, 2.0, 3.0)))
        );
    }

    #[test]
    fn test_read_vec2_u8_list() {
        let mut data = Vec::new();
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&0x22220001u32.to_le_bytes());
        data.push(50u8);
        data.push(100u8);

        let mut cursor = Cursor::new(data);
        let set = Section::read_non_string(&mut cursor, ValueKind::VEC2_U8_LIST).unwrap();

        assert_eq!(set.get(0x22220001), Some(&Value::Vec2U8([50, 100])));
    }

    #[test]
    fn test_read_string_list() {
        let mut data = Vec::new();
        data.extend_from_slice(&2u16.to_le_bytes());
        data.extend_from_slice(&0xAA000001u32.to_le_bytes());
        data.extend_from_slice(&0xAA000002u32.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&6u16.to_le_bytes());

        let string_data_offset = data.len() as u64;
        data.extend_from_slice(b"hello\0world\0");

        let mut cursor = Cursor::new(data);
        let set = Section::read_string_list(&mut cursor, string_data_offset).unwrap();

        assert_eq!(set.len(), 2);
        assert_eq!(
            set.get(0xAA000001),
            Some(&Value::String("hello".to_string()))
        );
        assert_eq!(
            set.get(0xAA000002),
            Some(&Value::String("world".to_string()))
        );
    }

    #[test]
    fn test_read_vec2_f32_list() {
        let mut data = Vec::new();
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&0x33330001u32.to_le_bytes());
        data.extend_from_slice(&4.0f32.to_le_bytes());
        data.extend_from_slice(&5.0f32.to_le_bytes());

        let mut cursor = Cursor::new(data);
        let set = Section::read_non_string(&mut cursor, ValueKind::VEC2_F32_LIST).unwrap();

        assert_eq!(
            set.get(0x33330001),
            Some(&Value::Vec2F32(Vec2::new(4.0, 5.0)))
        );
    }

    #[test]
    fn test_read_vec4_f32_list() {
        let mut data = Vec::new();
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&0x44440001u32.to_le_bytes());
        data.extend_from_slice(&1.0f32.to_le_bytes());
        data.extend_from_slice(&2.0f32.to_le_bytes());
        data.extend_from_slice(&3.0f32.to_le_bytes());
        data.extend_from_slice(&4.0f32.to_le_bytes());

        let mut cursor = Cursor::new(data);
        let set = Section::read_non_string(&mut cursor, ValueKind::VEC4_F32_LIST).unwrap();

        assert_eq!(
            set.get(0x44440001),
            Some(&Value::Vec4F32(Vec4::new(1.0, 2.0, 3.0, 4.0)))
        );
    }

    #[test]
    fn test_read_vec3_u8_list() {
        let mut data = Vec::new();
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&0x55550001u32.to_le_bytes());
        data.push(10);
        data.push(20);
        data.push(30);

        let mut cursor = Cursor::new(data);
        let set = Section::read_non_string(&mut cursor, ValueKind::VEC3_U8_LIST).unwrap();

        assert_eq!(set.get(0x55550001), Some(&Value::Vec3U8([10, 20, 30])));
    }

    #[test]
    fn test_read_int64_list() {
        let mut data = Vec::new();
        data.extend_from_slice(&2u16.to_le_bytes());
        data.extend_from_slice(&0x77770001u32.to_le_bytes());
        data.extend_from_slice(&0x77770002u32.to_le_bytes());
        data.extend_from_slice(&9999999999i64.to_le_bytes());
        data.extend_from_slice(&(-42i64).to_le_bytes());

        let mut cursor = Cursor::new(data);
        let set = Section::read_non_string(&mut cursor, ValueKind::INT64_LIST).unwrap();

        assert_eq!(set.len(), 2);
        assert_eq!(set.get(0x77770001), Some(&Value::I64(9999999999)));
        assert_eq!(set.get(0x77770002), Some(&Value::I64(-42)));
    }

    #[test]
    fn test_read_vec4_u8_list() {
        let mut data = Vec::new();
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&0x66660001u32.to_le_bytes());
        data.push(10);
        data.push(20);
        data.push(30);
        data.push(40);

        let mut cursor = Cursor::new(data);
        let set = Section::read_non_string(&mut cursor, ValueKind::VEC4_U8_LIST).unwrap();

        assert_eq!(set.get(0x66660001), Some(&Value::Vec4U8([10, 20, 30, 40])));
    }
}
