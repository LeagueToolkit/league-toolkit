use std::io::{Read, Seek, Write};

use byteorder::{LittleEndian as LE, ReadBytesExt, WriteBytesExt};
use indexmap::IndexMap;

use crate::error::{Error, Result};
use crate::section::Section;
use crate::value::{FromValue, Value};
use crate::value_flags::{ValueFlags, NON_STRING_KINDS};

/// Top-level inibin/troybin file container.
#[derive(Debug, Clone, PartialEq)]
pub struct Inibin {
    pub(crate) sections: IndexMap<ValueFlags, Section>,
}

impl Inibin {
    pub fn new() -> Self {
        Self {
            sections: IndexMap::new(),
        }
    }

    /// Parse an inibin file from a seekable reader.
    /// Supports version 1 (legacy) and version 2.
    pub fn from_reader<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        let version = reader.read_u8()?;
        match version {
            2 => Self::read_v2(reader),
            1 => Self::read_v1(reader),
            _ => Err(Error::UnsupportedVersion(version)),
        }
    }

    fn read_v2<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        let string_data_length = reader.read_u16::<LE>()?;
        let flags = ValueFlags::from_bits_truncate(reader.read_u16::<LE>()?);
        let mut sections = IndexMap::new();

        for &flag in &NON_STRING_KINDS {
            if flags.contains(flag) {
                let set = Section::read_non_string(reader, flag)?;
                sections.insert(flag, set);
            }
        }

        // StringList is always read last; validate string_data_length from the header
        if flags.contains(ValueFlags::STRING_LIST) {
            let count_pos = reader.stream_position()?;
            let value_count = reader.read_u16::<LE>()? as u64;
            reader.seek(std::io::SeekFrom::Start(count_pos))?;

            let string_data_offset = count_pos + 2 + value_count * 4 + value_count * 2;
            let set = Section::read_string_list(reader, string_data_offset)?;

            let actual = set.string_data_length();
            if actual != string_data_length {
                return Err(Error::StringDataLengthMismatch {
                    expected: string_data_length,
                    actual,
                });
            }

            sections.insert(ValueFlags::STRING_LIST, set);
        }

        Ok(Self { sections })
    }

    fn read_v1<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        let mut _padding = [0u8; 3];
        reader.read_exact(&mut _padding)?;

        let value_count = reader.read_u32::<LE>()? as usize;
        let _string_data_length = reader.read_u32::<LE>()?;

        let mut hashes = Vec::with_capacity(value_count);
        for _ in 0..value_count {
            hashes.push(reader.read_u32::<LE>()?);
        }

        let offsets_start = reader.stream_position()?;
        let string_data_offset = offsets_start + (value_count as u64) * 2;
        let set = Section::read_string_list_v1(reader, hashes, string_data_offset)?;

        let mut sections = IndexMap::new();
        sections.insert(ValueFlags::STRING_LIST, set);

        Ok(Self { sections })
    }

    /// Write as version 2 inibin format.
    pub fn to_writer<W: Write>(&self, writer: &mut W) -> Result<()> {
        let mut flags = ValueFlags::empty();
        for &flag in self.sections.keys() {
            flags |= flag;
        }

        let string_data_length = self
            .sections
            .get(&ValueFlags::STRING_LIST)
            .map(|s| s.string_data_length())
            .unwrap_or(0);

        // Header
        writer.write_u8(2)?;
        writer.write_u16::<LE>(string_data_length)?;
        writer.write_u16::<LE>(flags.bits())?;

        // Non-string sets in flag order
        for &flag in &NON_STRING_KINDS {
            if let Some(set) = self.sections.get(&flag) {
                if flag == ValueFlags::BIT_LIST {
                    set.write_bit_list(writer)?;
                } else {
                    set.write_non_string(writer)?;
                }
            }
        }

        // StringList last
        if let Some(set) = self.sections.get(&ValueFlags::STRING_LIST) {
            let string_data = set.write_string_list(writer)?;
            writer.write_all(&string_data)?;
        }

        Ok(())
    }

    // ── Key-based public API ───────────────────────────────────────

    pub fn get(&self, key: u32) -> Option<&Value> {
        for set in self.sections.values() {
            if let Some(value) = set.get(key) {
                return Some(value);
            }
        }
        None
    }

    /// Get a typed value by key, returning `None` on missing key or type mismatch.
    ///
    /// ```
    /// # use ltk_inibin::{Inibin, Value};
    /// let mut inibin = Inibin::new();
    /// inibin.insert(0x0001, 42i32);
    /// inibin.insert(0x0002, "hello");
    ///
    /// let v: Option<i32> = inibin.get_as(0x0001);
    /// assert_eq!(v, Some(42));
    ///
    /// let s: Option<&str> = inibin.get_as(0x0002);
    /// assert_eq!(s, Some("hello"));
    ///
    /// // Type mismatch returns None
    /// let wrong: Option<f32> = inibin.get_as(0x0001);
    /// assert_eq!(wrong, None);
    /// ```
    pub fn get_as<'a, T: FromValue<'a>>(&'a self, key: u32) -> Option<T> {
        self.get(key).and_then(T::from_inibin_value)
    }

    /// Get a typed value by key, returning `default` on missing key or type mismatch.
    ///
    /// ```
    /// # use ltk_inibin::Inibin;
    /// let mut inibin = Inibin::new();
    /// inibin.insert(0x0001, 42i32);
    ///
    /// assert_eq!(inibin.get_or(0x0001, 0i32), 42);
    /// assert_eq!(inibin.get_or(0x9999, 0i32), 0);   // missing key
    /// assert_eq!(inibin.get_or(0x0001, 0.0f32), 0.0); // type mismatch
    /// ```
    pub fn get_or<'a, T: FromValue<'a>>(&'a self, key: u32, default: T) -> T {
        self.get_as(key).unwrap_or(default)
    }

    pub fn contains_key(&self, key: u32) -> bool {
        self.get(key).is_some()
    }

    /// Insert or update a value, routing to the correct bucket by type.
    /// If the key exists in a different-type bucket, removes it first.
    ///
    /// ```
    /// # use ltk_inibin::Inibin;
    /// let mut inibin = Inibin::new();
    /// inibin.insert(0x0001, 42i32);
    /// inibin.insert(0x0002, "hello");
    /// ```
    pub fn insert(&mut self, key: u32, value: impl Into<Value>) {
        let value = value.into();
        let target_flags = value.flags();

        for (&flag, set) in self.sections.iter_mut() {
            if flag != target_flags {
                set.remove(key);
            }
        }

        self.sections
            .entry(target_flags)
            .or_insert_with(|| Section::new(target_flags))
            .insert(key, value);
    }

    pub fn remove(&mut self, key: u32) -> Option<Value> {
        for set in self.sections.values_mut() {
            if let Some(value) = set.remove(key) {
                return Some(value);
            }
        }
        None
    }

    pub fn len(&self) -> usize {
        self.sections.values().map(|s| s.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.sections.values().all(|s| s.is_empty())
    }

    pub fn iter(&self) -> impl Iterator<Item = (u32, &Value)> {
        self.sections.values().flat_map(|set| set.iter())
    }

    pub fn section(&self, flags: ValueFlags) -> Option<&Section> {
        self.sections.get(&flags)
    }

    pub fn section_mut(&mut self, flags: ValueFlags) -> Option<&mut Section> {
        self.sections.get_mut(&flags)
    }
}

impl Default for Inibin {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Cursor;

    #[test]
    fn test_unsupported_version() {
        let data = vec![3u8];
        let mut cursor = Cursor::new(data);
        let result = Inibin::from_reader(&mut cursor);
        assert!(matches!(result, Err(Error::UnsupportedVersion(3))));
    }

    #[test]
    fn test_read_v2_empty() {
        let mut data = Vec::new();
        data.push(2);
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());

        let mut cursor = Cursor::new(data);
        let file = Inibin::from_reader(&mut cursor).unwrap();
        assert!(file.sections.is_empty());
    }

    #[test]
    fn test_read_v2_with_int32() {
        let mut data = Vec::new();
        data.push(2);
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&(ValueFlags::INT32_LIST.bits()).to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&0x12345678u32.to_le_bytes());
        data.extend_from_slice(&99i32.to_le_bytes());

        let mut cursor = Cursor::new(data);
        let file = Inibin::from_reader(&mut cursor).unwrap();

        assert_eq!(file.get(0x12345678), Some(&Value::I32(99)));
    }

    #[test]
    fn test_read_v2_with_string_list() {
        let mut data = Vec::new();
        data.push(2);
        data.extend_from_slice(&6u16.to_le_bytes());
        data.extend_from_slice(&(ValueFlags::STRING_LIST.bits()).to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&0xAABBCCDDu32.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(b"hello\0");

        let mut cursor = Cursor::new(data);
        let file = Inibin::from_reader(&mut cursor).unwrap();

        assert_eq!(
            file.get(0xAABBCCDD),
            Some(&Value::String("hello".to_string()))
        );
    }

    #[test]
    fn test_read_v1() {
        let mut data = Vec::new();
        data.push(1);
        data.extend_from_slice(&[0u8; 3]);
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&4u32.to_le_bytes());
        data.extend_from_slice(&0x11223344u32.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(b"foo\0");

        let mut cursor = Cursor::new(data);
        let file = Inibin::from_reader(&mut cursor).unwrap();

        assert_eq!(
            file.get(0x11223344),
            Some(&Value::String("foo".to_string()))
        );
    }

    #[test]
    fn test_get_missing_key() {
        let file = Inibin::new();
        assert_eq!(file.get(0x12345678), None);
        assert!(!file.contains_key(0x12345678));
    }

    #[test]
    fn test_insert_and_get() {
        let mut file = Inibin::new();
        file.insert(0xABCD, Value::I32(42));
        assert_eq!(file.get(0xABCD), Some(&Value::I32(42)));
        assert!(file.contains_key(0xABCD));
    }

    #[test]
    fn test_insert_cross_bucket_migration() {
        let mut file = Inibin::new();
        file.insert(0xABCD, Value::I32(42));
        assert!(file.section(ValueFlags::INT32_LIST).is_some());

        file.insert(0xABCD, Value::F32(3.125));

        assert_eq!(file.get(0xABCD), Some(&Value::F32(3.125)));
        assert!(file
            .section(ValueFlags::INT32_LIST)
            .map(|s| s.get(0xABCD).is_none())
            .unwrap_or(true));
    }

    #[test]
    fn test_remove() {
        let mut file = Inibin::new();
        file.insert(0xABCD, Value::I32(42));

        let removed = file.remove(0xABCD);
        assert_eq!(removed, Some(Value::I32(42)));
        assert!(!file.contains_key(0xABCD));
    }

    #[test]
    fn test_round_trip_v2() {
        let mut file = Inibin::new();
        file.insert(0x0001, Value::I32(42));
        file.insert(0x0002, Value::F32(3.125));
        file.insert(0x0003, Value::I16(-100));
        file.insert(0x0004, Value::I8(255));
        file.insert(0x0005, Value::String("hello".to_string()));

        let mut buf = Vec::new();
        file.to_writer(&mut buf).unwrap();

        let mut cursor = Cursor::new(buf);
        let file2 = Inibin::from_reader(&mut cursor).unwrap();

        assert_eq!(file2.get(0x0001), Some(&Value::I32(42)));
        assert_eq!(file2.get(0x0002), Some(&Value::F32(3.125)));
        assert_eq!(file2.get(0x0003), Some(&Value::I16(-100)));
        assert_eq!(file2.get(0x0004), Some(&Value::I8(255)));
        assert_eq!(file2.get(0x0005), Some(&Value::String("hello".to_string())));
    }
}
