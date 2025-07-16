use super::super::ToImageError;

/// A decoded tex mipmap
pub struct TexSurface<'a> {
    pub width: u32,
    pub height: u32,
    pub data: TexSurfaceData<'a>,
}

pub enum TexSurfaceData<'a> {
    Bgra8Slice(&'a [u8]),
    Bgra8Owned(Vec<u32>),
}

impl TexSurface<'_> {
    pub fn into_rgba_image(self) -> Result<image::RgbaImage, ToImageError> {
        image::RgbaImage::from_raw(
            self.width,
            self.height,
            match self.data {
                TexSurfaceData::Bgra8Slice(data) => data
                    .chunks_exact(4)
                    .flat_map(|pixel| {
                        let [b, g, r, a] = pixel else {
                            unreachable!();
                        };
                        [r, g, b, a]
                    })
                    .copied()
                    .collect(),
                TexSurfaceData::Bgra8Owned(vec) => vec
                    .into_iter()
                    .flat_map(|pixel| {
                        let [b, g, r, a] = pixel.to_le_bytes();
                        [r, g, b, a]
                    })
                    .collect(),
            },
        )
        .ok_or(ToImageError::InvalidContainerSize)
    }
}
