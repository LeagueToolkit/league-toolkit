//! The `.ngrid_overlay` flag overlay and the `.rg_overlay` region tag overlay.
//!
//! Both formats hold up to 255 rectangles of per-cell values in grid cell coordinates.

use std::io::{ErrorKind, Read, Write};

use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use ltk_io_ext::untrusted::UntrustedCapacity;

use crate::{grid::element_count, CellFlags, Result, WriteError};

/// One rectangle of a [`NavGridOverlay`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct NavGridOverlayRect {
    /// The cell `x` of the first column. The value can be negative or past the grid.
    /// [`NavGrid::apply_overlay`](crate::NavGrid::apply_overlay) skips the cells outside the grid.
    pub x: i32,
    /// The cell `y` of the first row. The value can be negative or past the grid.
    pub y: i32,
    /// The number of columns.
    pub width: u32,
    /// The number of rows.
    pub height: u32,
    /// The cell flags in row-major order, `width * height` values.
    pub flags: Vec<CellFlags>,
}

/// A cell flag overlay: a `.ngrid_overlay` file.
///
/// [`NavGrid::apply_overlay`](crate::NavGrid::apply_overlay) writes the overlay into a grid.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct NavGridOverlay {
    /// The version byte. The game ignores it. Shipped files store 1.
    pub version: u8,
    /// The rectangles, at most 255.
    pub rects: Vec<NavGridOverlayRect>,
    /// The byte after the last rectangle. Shipped files store 0 or 1.
    pub trailing_byte: u8,
}

impl NavGridOverlay {
    /// Reads a flag overlay.
    ///
    /// # Errors
    ///
    /// Fails with [`ParseError::CountOverflow`](crate::ParseError::CountOverflow) if a rectangle
    /// size does not fit in memory.
    /// Fails with [`ParseError::Io`](crate::ParseError::Io) if the stream ends early or a read
    /// fails.
    pub fn from_reader<R: Read + ?Sized>(reader: &mut R) -> Result<Self> {
        let version = reader.read_u8()?;
        let count = reader.read_u8()?;
        let mut rects = Vec::with_capacity(count.into());
        for _ in 0..count {
            let x = reader.read_i32::<LE>()?;
            let y = reader.read_i32::<LE>()?;
            let width = reader.read_u32::<LE>()?;
            let height = reader.read_u32::<LE>()?;
            let len = element_count(width, height)?;
            let mut flags = Vec::with_untrusted_capacity(len);
            for _ in 0..len {
                flags.push(CellFlags::from_bits_retain(reader.read_u16::<LE>()?));
            }
            rects.push(NavGridOverlayRect {
                x,
                y,
                width,
                height,
                flags,
            });
        }
        let trailing_byte = reader.read_u8()?;
        Ok(Self {
            version,
            rects,
            trailing_byte,
        })
    }

    /// Writes the flag overlay.
    ///
    /// # Errors
    ///
    /// Fails with [`WriteError::TooManyRects`] if the overlay has more than 255 rectangles.
    /// Fails with [`WriteError::RectSizeMismatch`] if a rectangle does not hold
    /// `width * height` flags.
    /// Fails with [`WriteError::Io`] if a write fails.
    pub fn to_writer<W: Write + ?Sized>(&self, writer: &mut W) -> Result<(), WriteError> {
        let count = rect_count(self.rects.len())?;
        for (i, rect) in self.rects.iter().enumerate() {
            check_rect_size(i, rect.width.into(), rect.height.into(), rect.flags.len())?;
        }
        writer.write_u8(self.version)?;
        writer.write_u8(count)?;
        for rect in &self.rects {
            writer.write_i32::<LE>(rect.x)?;
            writer.write_i32::<LE>(rect.y)?;
            writer.write_u32::<LE>(rect.width)?;
            writer.write_u32::<LE>(rect.height)?;
            for flags in &rect.flags {
                writer.write_u16::<LE>(flags.bits())?;
            }
        }
        writer.write_u8(self.trailing_byte)?;
        Ok(())
    }
}

/// One rectangle of a [`RegionOverlay`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct RegionOverlayRect {
    /// The cell `x` of the first column.
    pub x: u16,
    /// The cell `y` of the first row.
    pub y: u16,
    /// The number of columns.
    pub width: u16,
    /// The number of rows.
    pub height: u16,
    /// The region tag words in row-major order, `width * height` values. Each word has the
    /// layout of [`NavGridCell::region_tags`](crate::NavGridCell::region_tags).
    pub region_tags: Vec<u32>,
}

/// A region tag overlay: a `.rg_overlay` file.
///
/// An empty file is an overlay without rectangles. An overlay without rectangles writes as an
/// empty file. [`NavGrid::apply_region_overlay`](crate::NavGrid::apply_region_overlay) writes the
/// overlay into a grid.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct RegionOverlay {
    /// The version byte. The game ignores it. Shipped files store 1.
    pub version: u8,
    /// The rectangles, at most 255.
    pub rects: Vec<RegionOverlayRect>,
}

impl RegionOverlay {
    /// Reads a region tag overlay. An empty stream reads as an overlay without rectangles and
    /// version 0.
    ///
    /// # Errors
    ///
    /// Fails with [`ParseError::Io`](crate::ParseError::Io) if the stream ends after the first
    /// byte and before the end of the last rectangle, or if a read fails.
    pub fn from_reader<R: Read + ?Sized>(reader: &mut R) -> Result<Self> {
        let version = match reader.read_u8() {
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(Self::default()),
            version => version?,
        };
        let count = reader.read_u8()?;
        let mut rects = Vec::with_capacity(count.into());
        for _ in 0..count {
            let x = reader.read_u16::<LE>()?;
            let y = reader.read_u16::<LE>()?;
            let width = reader.read_u16::<LE>()?;
            let height = reader.read_u16::<LE>()?;
            let len = usize::from(width) * usize::from(height);
            let mut region_tags = Vec::with_untrusted_capacity(len);
            for _ in 0..len {
                region_tags.push(reader.read_u32::<LE>()?);
            }
            rects.push(RegionOverlayRect {
                x,
                y,
                width,
                height,
                region_tags,
            });
        }
        Ok(Self { version, rects })
    }

    /// Writes the region tag overlay. Writes nothing if the overlay has no rectangles.
    ///
    /// # Errors
    ///
    /// Fails with [`WriteError::TooManyRects`] if the overlay has more than 255 rectangles.
    /// Fails with [`WriteError::RectSizeMismatch`] if a rectangle does not hold
    /// `width * height` region tag words.
    /// Fails with [`WriteError::Io`] if a write fails.
    pub fn to_writer<W: Write + ?Sized>(&self, writer: &mut W) -> Result<(), WriteError> {
        if self.rects.is_empty() {
            return Ok(());
        }
        let count = rect_count(self.rects.len())?;
        for (i, rect) in self.rects.iter().enumerate() {
            check_rect_size(
                i,
                rect.width.into(),
                rect.height.into(),
                rect.region_tags.len(),
            )?;
        }
        writer.write_u8(self.version)?;
        writer.write_u8(count)?;
        for rect in &self.rects {
            writer.write_u16::<LE>(rect.x)?;
            writer.write_u16::<LE>(rect.y)?;
            writer.write_u16::<LE>(rect.width)?;
            writer.write_u16::<LE>(rect.height)?;
            for &tags in &rect.region_tags {
                writer.write_u32::<LE>(tags)?;
            }
        }
        Ok(())
    }
}

/// Returns `len` as an 8-bit rectangle count.
fn rect_count(len: usize) -> Result<u8, WriteError> {
    u8::try_from(len).map_err(|_| WriteError::TooManyRects(len))
}

/// Fails if rectangle `rect` of `width` x `height` cells holds `actual` values.
fn check_rect_size(rect: usize, width: u64, height: u64, actual: usize) -> Result<(), WriteError> {
    let expected = width * height;
    if expected == actual as u64 {
        Ok(())
    } else {
        Err(WriteError::RectSizeMismatch {
            rect,
            expected,
            actual,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ParseError;

    fn ngrid_overlay_bytes() -> Vec<u8> {
        let mut out = vec![1, 2];
        for (x, y, w, h) in [(4i32, -5i32, 2u32, 1u32), (0, 0, 1, 2)] {
            out.extend(x.to_le_bytes());
            out.extend(y.to_le_bytes());
            out.extend(w.to_le_bytes());
            out.extend(h.to_le_bytes());
            for v in [0x0002u16, 0x00C2].iter().take((w * h) as usize) {
                out.extend(v.to_le_bytes());
            }
        }
        out.push(1);
        out
    }

    fn rg_overlay_bytes() -> Vec<u8> {
        let mut out = vec![1, 1];
        for v in [1u16, 2, 2, 1] {
            out.extend(v.to_le_bytes());
        }
        out.extend(0x1234_5678u32.to_le_bytes());
        out.extend(7u32.to_le_bytes());
        out
    }

    #[test]
    fn nav_grid_overlay_from_reader_reads_rects_and_trailing_byte() {
        let overlay = NavGridOverlay::from_reader(&mut &ngrid_overlay_bytes()[..]).unwrap();
        assert_eq!(overlay.version, 1);
        assert_eq!(overlay.trailing_byte, 1);
        assert_eq!(overlay.rects.len(), 2);
        assert_eq!((overlay.rects[0].x, overlay.rects[0].y), (4, -5));
        assert_eq!(overlay.rects[0].flags[1].bits(), 0x00C2);
    }

    #[test]
    fn nav_grid_overlay_to_writer_round_trips() {
        let bytes = ngrid_overlay_bytes();
        let overlay = NavGridOverlay::from_reader(&mut &bytes[..]).unwrap();
        let mut written = Vec::new();
        overlay.to_writer(&mut written).unwrap();
        assert_eq!(written, bytes);
    }

    #[test]
    fn nav_grid_overlay_from_reader_fails_without_trailing_byte() {
        let bytes = ngrid_overlay_bytes();
        let error = NavGridOverlay::from_reader(&mut &bytes[..bytes.len() - 1]).unwrap_err();
        assert!(matches!(error, ParseError::Io(e) if e.kind() == ErrorKind::UnexpectedEof));
    }

    #[test]
    fn nav_grid_overlay_to_writer_fails_on_rect_size_mismatch() {
        let overlay = NavGridOverlay {
            rects: vec![NavGridOverlayRect {
                width: 2,
                height: 2,
                flags: vec![CellFlags::WALL; 3],
                ..Default::default()
            }],
            ..Default::default()
        };
        let error = overlay.to_writer(&mut Vec::new()).unwrap_err();
        assert!(matches!(
            error,
            WriteError::RectSizeMismatch {
                rect: 0,
                expected: 4,
                actual: 3
            }
        ));
    }

    #[test]
    fn nav_grid_overlay_to_writer_fails_on_too_many_rects() {
        let overlay = NavGridOverlay {
            rects: vec![NavGridOverlayRect::default(); 256],
            ..Default::default()
        };
        let error = overlay.to_writer(&mut Vec::new()).unwrap_err();
        assert!(matches!(error, WriteError::TooManyRects(256)));
    }

    #[test]
    fn region_overlay_to_writer_round_trips() {
        let bytes = rg_overlay_bytes();
        let overlay = RegionOverlay::from_reader(&mut &bytes[..]).unwrap();
        assert_eq!(overlay.rects[0].region_tags, [0x1234_5678, 7]);
        let mut written = Vec::new();
        overlay.to_writer(&mut written).unwrap();
        assert_eq!(written, bytes);
    }

    #[test]
    fn region_overlay_from_reader_reads_empty_stream() {
        let overlay = RegionOverlay::from_reader(&mut &[][..]).unwrap();
        assert_eq!(overlay, RegionOverlay::default());
        let mut written = Vec::new();
        overlay.to_writer(&mut written).unwrap();
        assert!(written.is_empty());
    }

    #[test]
    fn region_overlay_from_reader_fails_on_truncated_rect() {
        let bytes = rg_overlay_bytes();
        let error = RegionOverlay::from_reader(&mut &bytes[..bytes.len() - 1]).unwrap_err();
        assert!(matches!(error, ParseError::Io(e) if e.kind() == ErrorKind::UnexpectedEof));
    }
}
