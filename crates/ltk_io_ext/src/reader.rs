use std::io::{self, Read};

use byteorder::{ByteOrder, ReadBytesExt};
use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
use ltk_primitives::{Color, Sphere, AABB};

#[derive(Debug, thiserror::Error)]
pub enum ReaderError {
    #[error("IO Error - {0}")]
    ReaderError(#[from] std::io::Error),
    #[error("UTF-8 Error - {0}")]
    Utf8Error(#[from] std::str::Utf8Error),
    #[error("From UTF-8 Error - {0}")]
    FromUtf8Error(#[from] std::string::FromUtf8Error),
}

pub type ReaderResult<T> = core::result::Result<T, ReaderError>;

pub trait ReaderExt: Read {
    fn read_padded_string<T: ByteOrder, const N: usize>(&mut self) -> ReaderResult<String> {
        let mut buf: [u8; N] = [0; N];
        self.read_exact(&mut buf)?;
        let i = buf.iter().position(|&b| b == b'\0').unwrap_or(buf.len());
        Ok(std::str::from_utf8(&buf[..i])?.to_string())
    }

    fn read_sized_string_u16<T: ByteOrder>(&mut self) -> ReaderResult<String> {
        let len = self.read_u16::<T>()?;
        let mut buf = vec![0; len as _];
        self.read_exact(&mut buf)?;
        Ok(String::from_utf8(buf)?)
    }

    fn read_sized_string_u32<T: ByteOrder>(&mut self) -> ReaderResult<String> {
        let len = self.read_u32::<T>()?;
        let mut buf = vec![0; len as _];
        self.read_exact(&mut buf)?;
        Ok(String::from_utf8(buf)?)
    }

    fn read_str_until_nul(&mut self) -> io::Result<String> {
        let mut s = String::new();
        loop {
            let c = self.read_u8()? as char;
            if c == b'\0' as char {
                break;
            }
            s.push(c);
        }
        Ok(s)
    }

    fn read_bool(&mut self) -> io::Result<bool> {
        Ok(self.read_u8()? != 0x0)
    }

    fn read_color_f32<O: ByteOrder>(&mut self) -> io::Result<Color<f32>> {
        Color::<f32>::from_reader::<O, _>(self)
    }
    fn read_color_u8(&mut self) -> io::Result<Color<u8>> {
        Color::<u8>::from_reader(self)
    }
    /// Reads color as BGRA u8 (4 bytes) - common in DirectX formats
    fn read_color_bgra_u8(&mut self) -> io::Result<Color<u8>> {
        Color::<u8>::from_reader_bgra(self)
    }
    /// Reads color as RGB u8 (3 bytes, alpha defaults to 255)
    fn read_color_rgb_u8(&mut self) -> io::Result<Color<u8>> {
        Color::<u8>::from_reader_rgb(self)
    }

    fn read_vec2<T: ByteOrder>(&mut self) -> io::Result<Vec2> {
        Ok(Vec2::new(self.read_f32::<T>()?, self.read_f32::<T>()?))
    }
    fn read_vec3<T: ByteOrder>(&mut self) -> io::Result<Vec3> {
        Ok(Vec3::new(
            self.read_f32::<T>()?,
            self.read_f32::<T>()?,
            self.read_f32::<T>()?,
        ))
    }
    fn read_vec4<T: ByteOrder>(&mut self) -> io::Result<Vec4> {
        Ok(Vec4::new(
            self.read_f32::<T>()?,
            self.read_f32::<T>()?,
            self.read_f32::<T>()?,
            self.read_f32::<T>()?,
        ))
    }

    fn read_quat<T: ByteOrder>(&mut self) -> io::Result<Quat> {
        Ok(Quat::from_array([
            self.read_f32::<T>()?,
            self.read_f32::<T>()?,
            self.read_f32::<T>()?,
            self.read_f32::<T>()?,
        ]))
    }

    /// Reads a 4x4 matrix whose 16 floats are its rows in order.
    ///
    /// A transform stored this way carries its translation in floats 3, 7 and 11. The
    /// returned [`Mat4`] holds those three in `w_axis`.
    fn read_mat4_row_major<T: ByteOrder>(&mut self) -> io::Result<Mat4> {
        Ok(Mat4::from_cols(
            self.read_vec4::<T>()?,
            self.read_vec4::<T>()?,
            self.read_vec4::<T>()?,
            self.read_vec4::<T>()?,
        )
        .transpose())
    }

    /// Reads a 4x4 matrix whose 16 floats are its columns in order.
    ///
    /// This is [`Mat4`]'s own storage order. A transform stored this way carries its
    /// translation in floats 12, 13 and 14.
    fn read_mat4_col_major<T: ByteOrder>(&mut self) -> io::Result<Mat4> {
        Ok(Mat4::from_cols(
            self.read_vec4::<T>()?,
            self.read_vec4::<T>()?,
            self.read_vec4::<T>()?,
            self.read_vec4::<T>()?,
        ))
    }

    fn read_aabb<T: ByteOrder>(&mut self) -> io::Result<AABB> {
        Ok(AABB {
            min: self.read_vec3::<T>()?,
            max: self.read_vec3::<T>()?,
        })
    }

    fn read_sphere<T: ByteOrder>(&mut self) -> io::Result<Sphere> {
        Ok(Sphere::new(self.read_vec3::<T>()?, self.read_f32::<T>()?))
    }
}

impl<R: Read + ?Sized> ReaderExt for R {}

#[cfg(test)]
mod tests {
    use super::*;
    use byteorder::LE;

    /// The 16 floats of a translation by (10, 20, 30), laid out column by column.
    fn translation_col_major() -> Vec<u8> {
        let mut floats = [0.0f32; 16];
        floats[0] = 1.0;
        floats[5] = 1.0;
        floats[10] = 1.0;
        floats[15] = 1.0;
        floats[12] = 10.0;
        floats[13] = 20.0;
        floats[14] = 30.0;
        floats.iter().flat_map(|f| f.to_le_bytes()).collect()
    }

    #[test]
    fn column_major_read_puts_translation_in_w_axis() {
        let bytes = translation_col_major();
        let mat = (&mut bytes.as_slice()).read_mat4_col_major::<LE>().unwrap();

        assert_eq!(mat.w_axis, glam::vec4(10.0, 20.0, 30.0, 1.0));
        assert_eq!(
            mat.transform_point3(Vec3::ZERO),
            Vec3::new(10.0, 20.0, 30.0)
        );
    }

    #[test]
    fn row_major_read_is_the_transpose_of_the_column_major_read() {
        let bytes = translation_col_major();
        let row = (&mut bytes.as_slice()).read_mat4_row_major::<LE>().unwrap();
        let col = (&mut bytes.as_slice()).read_mat4_col_major::<LE>().unwrap();

        assert_eq!(row, col.transpose());
        // The same bytes read row major put the translation in the last row, where a
        // transform consumer does not look for it.
        assert_eq!(row.w_axis, glam::vec4(0.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn column_major_write_round_trips() {
        use crate::WriterExt;

        let mat = Mat4::from_translation(Vec3::new(10.0, 20.0, 30.0));
        let mut bytes = Vec::new();
        bytes.write_mat4_col_major::<LE>(mat).unwrap();

        assert_eq!(bytes, translation_col_major());
        assert_eq!(
            (&mut bytes.as_slice()).read_mat4_col_major::<LE>().unwrap(),
            mat
        );
    }
}
