//! Other utilities (hashing, etc)

mod impls;

use std::{
    fmt::{Display, LowerHex},
    io::{Read, Write},
    num::ParseIntError,
    ops::{Deref, DerefMut},
    str::FromStr,
};

pub use impls::elf;
use impls::fnv1a;

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

use byteorder::{ByteOrder, ReadBytesExt as _, WriteBytesExt as _};
pub trait ReadBytesExt: Read {
    fn read_bin_hash<O: ByteOrder>(&mut self) -> std::io::Result<BinHash> {
        Ok(self.read_u32::<O>()?.into())
    }
    fn read_wad_hash<O: ByteOrder>(&mut self) -> std::io::Result<WadHash> {
        Ok(self.read_u64::<O>()?.into())
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
}

impl<W: Write + ?Sized> WriteBytesExt for W {}
