use std::{
    fmt::{Debug, Display},
    io::{self, Read, Write},
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use byteorder::{ByteOrder, ReadBytesExt, WriteBytesExt, LE};

mod private {
    pub trait Sealed {}

    impl Sealed for u16 {}
    impl Sealed for u32 {}
}
use private::Sealed;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, PartialEq, Eq)]
#[repr(transparent)]
/// Length-prefixed string. Can be either prefixed with a `u16` or `u32` (see [`Length`])
pub struct PrefixString<N: Length>(String, PhantomData<N>);

#[derive(Debug, thiserror::Error)]
pub enum StringReadError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    FromUtf8(#[from] std::string::FromUtf8Error),
    #[error("String too long")]
    LenOutOfRange,
}

#[derive(Debug, thiserror::Error)]
pub enum StringWriteError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("String too long")]
    LenOutOfRange,
}

impl<N: Length> PrefixString<N> {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into(), PhantomData)
    }

    pub fn from_reader(reader: &mut (impl Read + ?Sized)) -> Result<Self, StringReadError> {
        let len = N::read::<LE>(reader)?;
        let mut buf = vec![0; len.to_usize().ok_or(StringReadError::LenOutOfRange)?];
        reader.read_exact(&mut buf)?;
        Ok(Self::new(String::from_utf8(buf)?))
    }
    pub fn to_writer(&self, writer: &mut (impl Write + ?Sized)) -> Result<(), StringWriteError> {
        N::from_usize(self.0.len())
            .ok_or(StringWriteError::LenOutOfRange)?
            .write::<LE>(writer)?;
        writer.write_all(self.0.as_bytes())?;
        Ok(())
    }
}

impl<N: Length> Debug for PrefixString<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl<N: Length> Display for PrefixString<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl<N: Length> From<&str> for PrefixString<N> {
    fn from(value: &str) -> Self {
        Self::new(value.to_string())
    }
}
impl<N: Length> From<String> for PrefixString<N> {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}
impl<N: Length> From<PrefixString<N>> for String {
    fn from(value: PrefixString<N>) -> Self {
        value.0
    }
}

impl<N: Length> Deref for PrefixString<N> {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<N: Length> DerefMut for PrefixString<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Trait for types that can be used as length prefixes in a [`PrefixString`]
pub trait Length: Copy + Sized + Sealed {
    fn read<O: ByteOrder>(reader: &mut (impl Read + ?Sized)) -> io::Result<Self>;
    fn write<O: ByteOrder>(self, writer: &mut (impl Write + ?Sized)) -> io::Result<()>;

    fn from_usize(value: usize) -> Option<Self>;
    fn to_usize(self) -> Option<usize>;
}

impl Length for u16 {
    #[inline(always)]
    fn read<O: ByteOrder>(reader: &mut (impl Read + ?Sized)) -> io::Result<Self> {
        reader.read_u16::<O>()
    }

    #[inline(always)]
    fn write<O: ByteOrder>(self, writer: &mut (impl Write + ?Sized)) -> io::Result<()> {
        writer.write_u16::<O>(self)
    }

    #[inline(always)]
    fn to_usize(self) -> Option<usize> {
        Some(self.into())
    }

    #[inline(always)]
    fn from_usize(value: usize) -> Option<Self> {
        value.try_into().ok()
    }
}
impl Length for u32 {
    #[inline(always)]
    fn read<O: ByteOrder>(reader: &mut (impl Read + ?Sized)) -> io::Result<Self> {
        reader.read_u32::<O>()
    }

    #[inline(always)]
    fn write<O: ByteOrder>(self, writer: &mut (impl Write + ?Sized)) -> io::Result<()> {
        writer.write_u32::<O>(self)
    }

    #[inline(always)]
    fn to_usize(self) -> Option<usize> {
        self.try_into().ok()
    }

    #[inline(always)]
    fn from_usize(value: usize) -> Option<Self> {
        value.try_into().ok()
    }
}
