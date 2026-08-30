//! The leaf codecs: one per kind that decodes to a value rather than to a sub-view.
//!
//! Mechanical by design. Every one of these reads a fixed number of bytes at the cursor and
//! advances past them, so the only thing worth knowing is that they all fail the same way -
//! [`Error::IOError`] with [`std::io::ErrorKind::UnexpectedEof`] - when the slice ends first.

use glam::{Mat4, Vec2, Vec3, Vec4};
use ltk_hash::{BinHash, WadHash};
use ltk_primitives::Color;

use crate::{property::Kind, stream::layout::Cursor, Error};

impl Cursor<'_> {
    /// Reads one byte.
    ///
    /// # Errors
    ///
    /// See [`Cursor::take`].
    pub fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }

    /// Reads one byte as an `i8`.
    ///
    /// # Errors
    ///
    /// See [`Cursor::take`].
    pub fn i8(&mut self) -> Result<i8, Error> {
        Ok(self.u8()? as i8)
    }

    /// Reads a little-endian `u16`.
    ///
    /// # Errors
    ///
    /// See [`Cursor::take`].
    pub fn u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("2 bytes"),
        ))
    }

    /// Reads a little-endian `i16`.
    ///
    /// # Errors
    ///
    /// See [`Cursor::take`].
    pub fn i16(&mut self) -> Result<i16, Error> {
        Ok(self.u16()? as i16)
    }

    /// Reads a little-endian `u32`.
    ///
    /// # Errors
    ///
    /// See [`Cursor::take`].
    pub fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("4 bytes"),
        ))
    }

    /// Reads a little-endian `i32`.
    ///
    /// # Errors
    ///
    /// See [`Cursor::take`].
    pub fn i32(&mut self) -> Result<i32, Error> {
        Ok(self.u32()? as i32)
    }

    /// Reads a little-endian `u64`.
    ///
    /// # Errors
    ///
    /// See [`Cursor::take`].
    pub fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("8 bytes"),
        ))
    }

    /// Reads a little-endian `i64`.
    ///
    /// # Errors
    ///
    /// See [`Cursor::take`].
    pub fn i64(&mut self) -> Result<i64, Error> {
        Ok(self.u64()? as i64)
    }

    /// Reads a little-endian `f32`.
    ///
    /// # Errors
    ///
    /// See [`Cursor::take`].
    pub fn f32(&mut self) -> Result<f32, Error> {
        Ok(f32::from_bits(self.u32()?))
    }

    /// Reads one byte as a bool, any non-zero value being `true`.
    ///
    /// # Errors
    ///
    /// See [`Cursor::take`].
    pub fn bool(&mut self) -> Result<bool, Error> {
        Ok(self.u8()? != 0)
    }

    /// Reads two little-endian `f32`s.
    ///
    /// # Errors
    ///
    /// See [`Cursor::take`].
    pub fn vec2(&mut self) -> Result<Vec2, Error> {
        Ok(Vec2::new(self.f32()?, self.f32()?))
    }

    /// Reads three little-endian `f32`s.
    ///
    /// # Errors
    ///
    /// See [`Cursor::take`].
    pub fn vec3(&mut self) -> Result<Vec3, Error> {
        Ok(Vec3::new(self.f32()?, self.f32()?, self.f32()?))
    }

    /// Reads four little-endian `f32`s.
    ///
    /// # Errors
    ///
    /// See [`Cursor::take`].
    pub fn vec4(&mut self) -> Result<Vec4, Error> {
        Ok(Vec4::new(
            self.f32()?,
            self.f32()?,
            self.f32()?,
            self.f32()?,
        ))
    }

    /// Reads a 4×4 matrix of little-endian `f32`s, stored row by row.
    ///
    /// # Errors
    ///
    /// See [`Cursor::take`].
    pub fn mat4_row_major(&mut self) -> Result<Mat4, Error> {
        Ok(Mat4::from_cols(self.vec4()?, self.vec4()?, self.vec4()?, self.vec4()?).transpose())
    }

    /// Reads four bytes as an RGBA color.
    ///
    /// # Errors
    ///
    /// See [`Cursor::take`].
    pub fn color_u8(&mut self) -> Result<Color<u8>, Error> {
        let rgba = self.take(4)?;
        Ok(Color::new(rgba[0], rgba[1], rgba[2], rgba[3]))
    }

    /// Reads a [`BinHash`].
    ///
    /// # Errors
    ///
    /// See [`Cursor::take`].
    pub fn bin_hash(&mut self) -> Result<BinHash, Error> {
        Ok(BinHash(self.u32()?))
    }

    /// Reads a [`WadHash`].
    ///
    /// # Errors
    ///
    /// See [`Cursor::take`].
    pub fn wad_hash(&mut self) -> Result<WadHash, Error> {
        Ok(WadHash(self.u64()?))
    }

    /// Reads a property kind byte, under this cursor's
    /// [`Numbering`](crate::stream::layout::Numbering).
    ///
    /// # Errors
    ///
    /// [`Error::InvalidPropertyTypePrimitive`] if the byte does not decode under that
    /// numbering, or [`Error::IOError`] at the end of the slice.
    pub fn kind(&mut self) -> Result<Kind, Error> {
        let raw = self.u8()?;
        Kind::unpack(raw, self.numbering().is_legacy())
    }
}

impl<'a> Cursor<'a> {
    /// Reads a `u16`-length-prefixed UTF-8 string, borrowed from the slice.
    ///
    /// # Errors
    ///
    /// [`Error::Utf8Error`] if the bytes are not UTF-8, or [`Error::IOError`] at the end of
    /// the slice.
    pub fn str_u16(&mut self) -> Result<&'a str, Error> {
        let len = self.u16()? as usize;
        Ok(std::str::from_utf8(self.take(len)?)?)
    }
}
