//! Other utilities (hashing, etc)

mod impls;
mod value;

use std::{
    fmt::{Display, LowerHex},
    io::{Read, Write},
    num::ParseIntError,
    ops::{Deref, DerefMut},
    str::FromStr,
};

pub use impls::elf;
pub use impls::fnv1;
use impls::fnv1a;
pub use impls::sdbm;
pub use value::{HashAlgorithm, HashValue, HashWidth, Hasher};

use xxhash_rust::xxh64::xxh64;

pub trait Hash: std::hash::Hash + Eq + Ord + Copy {
    fn hash_str(src: impl AsRef<str>) -> Self;
}

/// Wad path hashes - case insensitive
#[repr(transparent)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, std::hash::Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WadHash(pub u64);

impl Hash for WadHash {
    fn hash_str(src: impl AsRef<str>) -> Self {
        Self(xxh64(src.as_ref().to_ascii_lowercase().as_bytes(), 0))
    }
}

impl Display for WadHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        LowerHex::fmt(self, f)
    }
}
impl LowerHex for WadHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        LowerHex::fmt(&self.0, f)
    }
}
impl FromStr for WadHash {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str_radix(s, 16)
    }
}
impl WadHash {
    pub const fn from_str_radix(src: &str, radix: u32) -> Result<Self, ParseIntError> {
        match u64::from_str_radix(src, radix) {
            Ok(v) => Ok(Self(v)),
            Err(e) => Err(e),
        }
    }
}

impl From<&str> for WadHash {
    fn from(value: &str) -> Self {
        Self::hash_str(value)
    }
}

impl From<u64> for WadHash {
    fn from(value: u64) -> Self {
        Self(value)
    }
}
impl AsRef<u64> for WadHash {
    fn as_ref(&self) -> &u64 {
        &self.0
    }
}
impl AsMut<u64> for WadHash {
    fn as_mut(&mut self) -> &mut u64 {
        &mut self.0
    }
}
impl Deref for WadHash {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}
impl DerefMut for WadHash {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut()
    }
}

/// Used for bin field/class/property names, etc. - case insensitive
#[repr(transparent)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, std::hash::Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BinHash(pub u32);

impl Hash for BinHash {
    fn hash_str(src: impl AsRef<str>) -> Self {
        Self(fnv1a::hash_lower(src.as_ref()))
    }
}

impl Display for BinHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        LowerHex::fmt(self, f)
    }
}
impl LowerHex for BinHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        LowerHex::fmt(&self.0, f)
    }
}
impl FromStr for BinHash {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str_radix(s, 16)
    }
}
impl BinHash {
    pub const fn from_str_radix(src: &str, radix: u32) -> Result<Self, ParseIntError> {
        match u32::from_str_radix(src, radix) {
            Ok(v) => Ok(Self(v)),
            Err(e) => Err(e),
        }
    }
}

impl From<&str> for BinHash {
    fn from(value: &str) -> Self {
        Self::hash_str(value)
    }
}

impl From<u32> for BinHash {
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl AsRef<u32> for BinHash {
    fn as_ref(&self) -> &u32 {
        &self.0
    }
}
impl AsMut<u32> for BinHash {
    fn as_mut(&mut self) -> &mut u32 {
        &mut self.0
    }
}
impl Deref for BinHash {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}
impl DerefMut for BinHash {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut()
    }
}

/// The key of a value in an inibin file - case insensitive
///
/// The key is the SDBM hash of the section name, the `*` character and the property name.
///
/// ```
/// use ltk_hash::{Hash as _, IniHash};
///
/// let key = IniHash::new("System", "GroupPart1");
/// assert_eq!(key, IniHash(0x0616933a));
/// assert_eq!(key, IniHash::hash_str("system*grouppart1"));
/// ```
#[repr(transparent)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, std::hash::Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IniHash(pub u32);

impl Hash for IniHash {
    fn hash_str(src: impl AsRef<str>) -> Self {
        Self(sdbm::hash_lower(src.as_ref()))
    }
}

impl Display for IniHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        LowerHex::fmt(self, f)
    }
}
impl LowerHex for IniHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        LowerHex::fmt(&self.0, f)
    }
}
impl FromStr for IniHash {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str_radix(s, 16)
    }
}
impl IniHash {
    /// Returns the key of the property `name` in the section `section`.
    pub const fn new(section: &str, name: &str) -> Self {
        let hash = sdbm::hash_lower(section);
        let hash = sdbm::hash_lower_from(hash, "*");
        Self(sdbm::hash_lower_from(hash, name))
    }

    pub const fn from_str_radix(src: &str, radix: u32) -> Result<Self, ParseIntError> {
        match u32::from_str_radix(src, radix) {
            Ok(v) => Ok(Self(v)),
            Err(e) => Err(e),
        }
    }
}

impl From<&str> for IniHash {
    fn from(value: &str) -> Self {
        Self::hash_str(value)
    }
}

impl From<u32> for IniHash {
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl AsRef<u32> for IniHash {
    fn as_ref(&self) -> &u32 {
        &self.0
    }
}
impl AsMut<u32> for IniHash {
    fn as_mut(&mut self) -> &mut u32 {
        &mut self.0
    }
}
impl Deref for IniHash {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}
impl DerefMut for IniHash {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut()
    }
}

use byteorder::{ByteOrder, ReadBytesExt as _, WriteBytesExt as _};
pub trait ReadBytesExt: Read {
    fn read_bin_hash<O: ByteOrder>(&mut self) -> std::io::Result<BinHash> {
        Ok(self.read_u32::<O>()?.into())
    }
    fn read_wad_hash<O: ByteOrder>(&mut self) -> std::io::Result<WadHash> {
        Ok(self.read_u64::<O>()?.into())
    }
    fn read_ini_hash<O: ByteOrder>(&mut self) -> std::io::Result<IniHash> {
        Ok(self.read_u32::<O>()?.into())
    }
    /// Reads a [`HashValue`] of `width` bytes.
    fn read_hash_value<O: ByteOrder>(&mut self, width: HashWidth) -> std::io::Result<HashValue> {
        Ok(match width {
            HashWidth::W4 => HashValue::narrow(self.read_u32::<O>()?),
            HashWidth::W8 => HashValue::wide(self.read_u64::<O>()?),
        })
    }
}

impl<R: Read + ?Sized> ReadBytesExt for R {}
pub trait WriteBytesExt: Write {
    fn write_bin_hash<O: ByteOrder>(&mut self, hash: BinHash) -> std::io::Result<()> {
        self.write_u32::<O>(*hash)
    }
    fn write_wad_hash<O: ByteOrder>(&mut self, hash: WadHash) -> std::io::Result<()> {
        self.write_u64::<O>(*hash)
    }
    fn write_ini_hash<O: ByteOrder>(&mut self, hash: IniHash) -> std::io::Result<()> {
        self.write_u32::<O>(*hash)
    }
    /// Writes `hash` in `hash.width()` bytes.
    fn write_hash_value<O: ByteOrder>(&mut self, hash: HashValue) -> std::io::Result<()> {
        match hash.width() {
            HashWidth::W4 => self.write_u32::<O>(hash.as_u32()),
            HashWidth::W8 => self.write_u64::<O>(hash.as_u64()),
        }
    }
}

impl<W: Write + ?Sized> WriteBytesExt for W {}
