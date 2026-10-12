use std::{
    collections::{btree_map, BTreeMap},
    io::{self, Read, Write},
};

use byteorder::{ReadBytesExt as _, WriteBytesExt as _, LE};
use glam::{Vec2, Vec3, Vec4};
use ltk_hash::{IniHash, ReadBytesExt as _, WriteBytesExt as _};

use crate::{Error, Result, Section, SectionMut, Value, ValueKind};

#[cfg(test)]
mod tests;

const VERSION: u8 = 2;

/// The values of an inibin file, indexed by key.
///
/// `.inibin`, `.troybin` and `.cfgbin` files have the same format. Each file is the compiled form
/// of an ini text file. A key is the hash of a section name and a property name
/// ([`IniHash::new`]). The file does not store the names.
///
/// An `Inibin` stores one value per key and iterates in ascending key order.
/// [`section`](Self::section) and [`section_mut`](Self::section_mut) access values by section
/// name and property name. The other methods access values by key.
///
/// ```
/// use ltk_ini::{IniHash, Inibin};
///
/// let mut inibin = Inibin::new();
/// inibin.section_mut("System").insert("GroupPart0", "Flash");
/// inibin.section_mut("Flash").insert("p-life", 1.5);
///
/// let mut bytes = Vec::new();
/// inibin.to_writer(&mut bytes)?;
///
/// let inibin = Inibin::from_reader(&mut bytes.as_slice())?;
/// assert_eq!(inibin.section("System").get_str("GroupPart0"), Some("Flash"));
/// assert_eq!(inibin.section("Flash").get_f32("p-life"), Some(1.5));
/// assert_eq!(inibin.get_f32(IniHash::new("Flash", "p-life")), Some(1.5));
/// # Ok::<(), ltk_ini::Error>(())
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Inibin {
    values: BTreeMap<IniHash, Value>,
}

impl Inibin {
    /// Returns an `Inibin` with no values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of values.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns `true` if the `Inibin` has no values.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Returns the value of `key`. Returns `None` if the `Inibin` has no value for `key`.
    #[must_use]
    pub fn get(&self, key: impl Into<IniHash>) -> Option<&Value> {
        self.values.get(&key.into())
    }

    /// Returns a mutable reference to the value of `key`. Returns `None` if the `Inibin` has no
    /// value for `key`.
    #[must_use]
    pub fn get_mut(&mut self, key: impl Into<IniHash>) -> Option<&mut Value> {
        self.values.get_mut(&key.into())
    }

    /// Returns `true` if the `Inibin` has a value for `key`.
    #[must_use]
    pub fn contains_key(&self, key: impl Into<IniHash>) -> bool {
        self.values.contains_key(&key.into())
    }

    /// Returns a view of the values of the section `name`.
    ///
    /// The section name is not case sensitive. The view contains no values if the `Inibin` has
    /// no value in the section `name`.
    #[must_use]
    pub fn section(&self, name: &str) -> Section<'_> {
        Section::new(self, name)
    }

    /// Returns a mutable view of the values of the section `name`.
    pub fn section_mut(&mut self, name: &str) -> SectionMut<'_> {
        SectionMut::new(self, name)
    }

    /// Returns the string of `key`. Returns `None` if the value is not a [`Value::String`].
    #[must_use]
    pub fn get_str(&self, key: impl Into<IniHash>) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    /// Returns the value of `key` as the 32-bit integer that the client reads. Converts the
    /// value with [`Value::to_i32`].
    #[must_use]
    pub fn get_i32(&self, key: impl Into<IniHash>) -> Option<i32> {
        self.get(key).and_then(Value::to_i32)
    }

    /// Returns the value of `key` as the 64-bit integer that the client reads. Converts the
    /// value with [`Value::to_i64`].
    #[must_use]
    pub fn get_i64(&self, key: impl Into<IniHash>) -> Option<i64> {
        self.get(key).and_then(Value::to_i64)
    }

    /// Returns the value of `key` as the float that the client reads. Converts the value with
    /// [`Value::to_f32`].
    #[must_use]
    pub fn get_f32(&self, key: impl Into<IniHash>) -> Option<f32> {
        self.get(key).and_then(Value::to_f32)
    }

    /// Returns the value of `key` as the boolean that the client reads. Converts the value with
    /// [`Value::to_bool`].
    #[must_use]
    pub fn get_bool(&self, key: impl Into<IniHash>) -> Option<bool> {
        self.get(key).and_then(Value::to_bool)
    }

    /// Returns the value of `key` as the 2 floats that the client reads. Converts the value with
    /// [`Value::to_vec2`].
    #[must_use]
    pub fn get_vec2(&self, key: impl Into<IniHash>) -> Option<Vec2> {
        self.get(key).and_then(Value::to_vec2)
    }

    /// Returns the value of `key` as the 3 floats that the client reads. Converts the value with
    /// [`Value::to_vec3`].
    #[must_use]
    pub fn get_vec3(&self, key: impl Into<IniHash>) -> Option<Vec3> {
        self.get(key).and_then(Value::to_vec3)
    }

    /// Returns the value of `key` as the 4 floats that the client reads. Converts the value with
    /// [`Value::to_vec4`].
    #[must_use]
    pub fn get_vec4(&self, key: impl Into<IniHash>) -> Option<Vec4> {
        self.get(key).and_then(Value::to_vec4)
    }

    /// Sets the value of `key` to `value`. Returns the previous value of `key`.
    ///
    /// Replaces the previous value if it has a different storage type.
    pub fn insert(&mut self, key: impl Into<IniHash>, value: impl Into<Value>) -> Option<Value> {
        self.values.insert(key.into(), value.into())
    }

    /// Removes the value of `key`. Returns the removed value. Returns `None` if the `Inibin` has
    /// no value for `key`.
    pub fn remove(&mut self, key: impl Into<IniHash>) -> Option<Value> {
        self.values.remove(&key.into())
    }

    /// Removes each value for which `keep` returns `false`.
    pub fn retain(&mut self, mut keep: impl FnMut(IniHash, &mut Value) -> bool) {
        self.values.retain(|key, value| keep(*key, value));
    }

    /// Removes every value.
    pub fn clear(&mut self) {
        self.values.clear();
    }

    /// Returns an iterator over the keys and values, in ascending key order.
    pub fn iter(&self) -> btree_map::Iter<'_, IniHash, Value> {
        self.values.iter()
    }

    /// Returns an iterator over the keys and mutable values, in ascending key order.
    pub fn iter_mut(&mut self) -> btree_map::IterMut<'_, IniHash, Value> {
        self.values.iter_mut()
    }

    /// Returns an iterator over the keys, in ascending order.
    pub fn keys(&self) -> btree_map::Keys<'_, IniHash, Value> {
        self.values.keys()
    }

    /// Returns an iterator over the values, in ascending key order.
    pub fn values(&self) -> btree_map::Values<'_, IniHash, Value> {
        self.values.values()
    }

    /// Reads an inibin file of version 1 or version 2.
    ///
    /// Reads from the version byte to the end of the string data. Does not read bytes after the
    /// string data.
    ///
    /// A version 1 file stores every value as a string. Returns each value of a version 1 file
    /// as a [`Value::String`]. The typed getters convert the string.
    ///
    /// # Errors
    ///
    /// Fails with [`Error::UnsupportedVersion`] if the version byte is not 1 or 2. Fails with
    /// [`Error::DuplicateKey`] if the file stores a key more than once. Fails if a string starts
    /// outside the string data, has no NUL terminator or is not valid UTF-8.
    pub fn from_reader<R: Read + ?Sized>(reader: &mut R) -> Result<Self> {
        match reader.read_u8()? {
            1 => Self::read_v1(reader),
            VERSION => Self::read_v2(reader),
            version => Err(Error::UnsupportedVersion(version)),
        }
    }

    fn read_v1<R: Read + ?Sized>(reader: &mut R) -> Result<Self> {
        let mut padding = [0; 3];
        reader.read_exact(&mut padding)?;
        let count = reader.read_u32::<LE>()?;
        let string_data_len = reader.read_u32::<LE>()?;

        let mut string_offsets = Vec::new();
        for _ in 0..count {
            let key = reader.read_ini_hash::<LE>()?;
            string_offsets.push((key, reader.read_u32::<LE>()?));
        }

        // `string_data_len` is untrusted. `read_to_end` allocates only for the bytes that `reader`
        // returns.
        let mut string_data = Vec::new();
        reader
            .take(u64::from(string_data_len))
            .read_to_end(&mut string_data)?;
        if string_data.len() != string_data_len as usize {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
        }

        let mut inibin = Self::new();
        inibin.insert_strings(&string_data, string_offsets)?;
        Ok(inibin)
    }

    fn read_v2<R: Read + ?Sized>(reader: &mut R) -> Result<Self> {
        let string_data_len = reader.read_u16::<LE>()?;
        let flags = reader.read_u16::<LE>()?;

        let mut inibin = Self::new();
        let mut string_offsets = Vec::new();
        for kind in ValueKind::ALL {
            if flags & kind.flag() == 0 {
                continue;
            }

            let count = usize::from(reader.read_u16::<LE>()?);
            let mut keys = Vec::with_capacity(count);
            for _ in 0..count {
                keys.push(reader.read_ini_hash::<LE>()?);
            }

            match kind {
                ValueKind::Bool => {
                    let mut bits = vec![0; count.div_ceil(8)];
                    reader.read_exact(&mut bits)?;
                    for (index, key) in keys.into_iter().enumerate() {
                        let bit = bits[index / 8] >> (index % 8) & 1;
                        inibin.insert_new(key, Value::Bool(bit != 0))?;
                    }
                }
                // The string data follows the last value set. `insert_strings` resolves the
                // offsets after the loop.
                ValueKind::String => {
                    for key in keys {
                        string_offsets.push((key, u32::from(reader.read_u16::<LE>()?)));
                    }
                }
                _ => {
                    for key in keys {
                        let value = read_value(reader, kind)?;
                        inibin.insert_new(key, value)?;
                    }
                }
            }
        }

        let mut string_data = vec![0; usize::from(string_data_len)];
        reader.read_exact(&mut string_data)?;
        inibin.insert_strings(&string_data, string_offsets)?;

        Ok(inibin)
    }

    /// Inserts a [`Value::String`] for each key of `string_offsets`. Reads the string from
    /// `string_data` at the offset of the key.
    fn insert_strings(
        &mut self,
        string_data: &[u8],
        string_offsets: Vec<(IniHash, u32)>,
    ) -> Result<()> {
        for (key, offset) in string_offsets {
            let string =
                string_data
                    .get(offset as usize..)
                    .ok_or(Error::StringOffsetOutOfBounds {
                        key,
                        offset,
                        len: string_data.len() as u32,
                    })?;
            let len = string
                .iter()
                .position(|&byte| byte == 0)
                .ok_or(Error::UnterminatedString { key })?;
            let string = std::str::from_utf8(&string[..len])
                .map_err(|source| Error::InvalidUtf8 { key, source })?;
            self.insert_new(key, Value::String(string.to_owned()))?;
        }
        Ok(())
    }

    fn insert_new(&mut self, key: IniHash, value: Value) -> Result<()> {
        match self.values.insert(key, value) {
            None => Ok(()),
            Some(_) => Err(Error::DuplicateKey(key)),
        }
    }

    /// Writes the `Inibin` as a version 2 inibin file.
    ///
    /// Writes the keys of each storage type in ascending order. Stores equal strings once.
    /// Stores a string that is a suffix of another string inside that string.
    ///
    /// # Errors
    ///
    /// Fails with [`Error::TooManyValues`] if more than 65535 values have the same storage type.
    /// Fails with [`Error::StringDataTooLong`] if the string data is longer than 65535 bytes.
    /// Fails with [`Error::NulInString`] if a string contains a NUL byte.
    pub fn to_writer<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        let mut sets: [Vec<(IniHash, &Value)>; ValueKind::ALL.len()] = Default::default();
        for (key, value) in &self.values {
            sets[value.kind() as usize].push((*key, value));
        }

        let mut flags = 0;
        for (kind, set) in ValueKind::ALL.into_iter().zip(&sets) {
            if u16::try_from(set.len()).is_err() {
                return Err(Error::TooManyValues {
                    kind,
                    count: set.len(),
                });
            }
            if !set.is_empty() {
                flags |= kind.flag();
            }
        }

        let mut strings = Vec::new();
        for &(key, value) in &sets[ValueKind::String as usize] {
            let string = value.as_str().unwrap_or_default();
            if string.contains('\0') {
                return Err(Error::NulInString { key });
            }
            strings.push(string);
        }
        let (string_data, string_offsets) = layout_strings(&strings);
        let string_data_len =
            u16::try_from(string_data.len()).map_err(|_| Error::StringDataTooLong {
                len: string_data.len(),
            })?;

        writer.write_u8(VERSION)?;
        writer.write_u16::<LE>(string_data_len)?;
        writer.write_u16::<LE>(flags)?;

        for (kind, set) in ValueKind::ALL.into_iter().zip(&sets) {
            if set.is_empty() {
                continue;
            }

            writer.write_u16::<LE>(set.len() as u16)?;
            for &(key, _) in set {
                writer.write_ini_hash::<LE>(key)?;
            }

            match kind {
                ValueKind::Bool => {
                    let mut bits = vec![0u8; set.len().div_ceil(8)];
                    for (index, (_, value)) in set.iter().enumerate() {
                        if matches!(value, Value::Bool(true)) {
                            bits[index / 8] |= 1 << (index % 8);
                        }
                    }
                    writer.write_all(&bits)?;
                }
                ValueKind::String => {
                    for &offset in &string_offsets {
                        writer.write_u16::<LE>(offset as u16)?;
                    }
                }
                _ => {
                    for (_, value) in set {
                        write_value(writer, value)?;
                    }
                }
            }
        }

        writer.write_all(&string_data)?;
        Ok(())
    }
}

/// Returns the string data of `strings` and the offset of each string in the string data.
///
/// The string data contains each string that is not a suffix of another string, in the order of
/// `strings`, with a NUL terminator. The offset of a string that is a suffix of another string is
/// inside that string. The returned string data has the minimum length.
fn layout_strings(strings: &[&str]) -> (Vec<u8>, Vec<usize>) {
    // `a` is a suffix of `b` if the reversed bytes of `a` are a prefix of the reversed bytes of
    // `b`. In the sort order of the reversed bytes, a string that is a suffix of another string
    // is directly before a string that it is a suffix of.
    let mut order: Vec<usize> = (0..strings.len()).collect();
    order.sort_by(|&a, &b| {
        let reversed = |index: usize| strings[index].bytes().rev();
        reversed(a).cmp(reversed(b)).then(b.cmp(&a))
    });

    // `holders[i]` is the index of the string that contains the bytes of `strings[i]`.
    let mut holders: Vec<usize> = (0..strings.len()).collect();
    for pair in order.windows(2).rev() {
        if strings[pair[1]].ends_with(strings[pair[0]]) {
            holders[pair[0]] = holders[pair[1]];
        }
    }

    let mut data = Vec::new();
    let mut offsets = vec![0; strings.len()];
    for (index, string) in strings.iter().enumerate() {
        if holders[index] == index {
            offsets[index] = data.len();
            data.extend_from_slice(string.as_bytes());
            data.push(0);
        }
    }
    for (index, string) in strings.iter().enumerate() {
        let holder = holders[index];
        offsets[index] = offsets[holder] + strings[holder].len() - string.len();
    }
    (data, offsets)
}

fn read_fixed<R: Read + ?Sized, const N: usize>(reader: &mut R) -> Result<[u8; N]> {
    let mut fixed = [0; N];
    reader.read_exact(&mut fixed)?;
    Ok(fixed)
}

fn read_floats<R: Read + ?Sized, const N: usize>(reader: &mut R) -> Result<[f32; N]> {
    let mut floats = [0.0; N];
    reader.read_f32_into::<LE>(&mut floats)?;
    Ok(floats)
}

/// Reads one value of the storage type `kind`. `kind` must have a fixed value size.
fn read_value<R: Read + ?Sized>(reader: &mut R, kind: ValueKind) -> Result<Value> {
    Ok(match kind {
        ValueKind::I32 => Value::I32(reader.read_i32::<LE>()?),
        ValueKind::F32 => Value::F32(reader.read_f32::<LE>()?),
        ValueKind::Fixed => Value::Fixed(reader.read_u8()?),
        ValueKind::I16 => Value::I16(reader.read_i16::<LE>()?),
        ValueKind::U8 => Value::U8(reader.read_u8()?),
        ValueKind::FixedVec3 => Value::FixedVec3(read_fixed(reader)?),
        ValueKind::Vec3 => Value::Vec3(Vec3::from_array(read_floats(reader)?)),
        ValueKind::FixedVec2 => Value::FixedVec2(read_fixed(reader)?),
        ValueKind::Vec2 => Value::Vec2(Vec2::from_array(read_floats(reader)?)),
        ValueKind::FixedVec4 => Value::FixedVec4(read_fixed(reader)?),
        ValueKind::Vec4 => Value::Vec4(Vec4::from_array(read_floats(reader)?)),
        ValueKind::I64 => Value::I64(reader.read_i64::<LE>()?),
        ValueKind::Bool | ValueKind::String => {
            unreachable!("a {kind:?} value has no fixed size")
        }
    })
}

fn write_floats<W: Write + ?Sized, const N: usize>(writer: &mut W, floats: [f32; N]) -> Result<()> {
    for float in floats {
        writer.write_f32::<LE>(float)?;
    }
    Ok(())
}

/// Writes `value`. The storage type of `value` must have a fixed value size.
fn write_value<W: Write + ?Sized>(writer: &mut W, value: &Value) -> Result<()> {
    match value {
        Value::I32(value) => writer.write_i32::<LE>(*value)?,
        Value::F32(value) => writer.write_f32::<LE>(*value)?,
        Value::Fixed(fixed) => writer.write_u8(*fixed)?,
        Value::I16(value) => writer.write_i16::<LE>(*value)?,
        Value::U8(value) => writer.write_u8(*value)?,
        Value::FixedVec3(fixed) => writer.write_all(fixed)?,
        Value::Vec3(value) => write_floats(writer, value.to_array())?,
        Value::FixedVec2(fixed) => writer.write_all(fixed)?,
        Value::Vec2(value) => write_floats(writer, value.to_array())?,
        Value::FixedVec4(fixed) => writer.write_all(fixed)?,
        Value::Vec4(value) => write_floats(writer, value.to_array())?,
        Value::I64(value) => writer.write_i64::<LE>(*value)?,
        Value::Bool(_) | Value::String(_) => {
            unreachable!("a {:?} value has no fixed size", value.kind())
        }
    }
    Ok(())
}

impl<K: Into<IniHash>, V: Into<Value>> FromIterator<(K, V)> for Inibin {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut inibin = Self::new();
        inibin.extend(iter);
        inibin
    }
}

impl<K: Into<IniHash>, V: Into<Value>> Extend<(K, V)> for Inibin {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        for (key, value) in iter {
            self.insert(key, value);
        }
    }
}

impl IntoIterator for Inibin {
    type Item = (IniHash, Value);
    type IntoIter = btree_map::IntoIter<IniHash, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.values.into_iter()
    }
}

impl<'a> IntoIterator for &'a Inibin {
    type Item = (&'a IniHash, &'a Value);
    type IntoIter = btree_map::Iter<'a, IniHash, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut Inibin {
    type Item = (&'a IniHash, &'a mut Value);
    type IntoIter = btree_map::IterMut<'a, IniHash, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
