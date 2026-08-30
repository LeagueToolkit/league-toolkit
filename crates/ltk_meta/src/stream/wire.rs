//! The byte-level wire core: one module that knows how property values lay out.
//!
//! Every layer that touches value bytes — the streaming sweep, the buffered object views, and
//! eventually the eager readers — renders over what is defined here, so two surfaces can never
//! disagree about the same bytes. The rules mirror the client's `MetaValue_skipByType`:
//! primitives have fixed widths, strings a `u16` length prefix, complex values carry their byte
//! size ahead of their body, [`Kind::Optional`] recurses into its zero-or-one element, and
//! [`Kind::BitBool`] is one byte.
//!
//! Nothing here allocates or decodes value contents to move past a value. Decoding happens only
//! when a leaf codec on [`Cursor`] is asked for, and the walk that does parse
//! ([`walk_value`]) trusts counts, using declared sizes only to realign — a mismatch is
//! recorded in a [`DiscrepancyLog`], not raised (see [`SizeDiscrepancy`]).

use ltk_hash::{BinHash, WadHash};

use crate::{
    path::ValueShape,
    property::Kind,
    stream::{DiscrepancyLog, SizeDiscrepancy},
    Error,
};

/// A reading position over a byte slice.
///
/// The wire core's functions advance one of these; the slice itself is never copied, so a
/// cursor is `Copy` and forking one is free. Reading past the end of the slice fails with
/// [`Error::IOError`] carrying [`std::io::ErrorKind::UnexpectedEof`].
#[derive(Debug, Clone, Copy)]
pub struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    /// A cursor at the start of `buf`.
    #[must_use]
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    /// The current position, in bytes from the start of the slice.
    #[must_use]
    pub fn position(&self) -> usize {
        self.pos
    }

    /// How many bytes remain ahead of the cursor.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    /// The next `n` bytes, advancing past them.
    ///
    /// # Errors
    ///
    /// [`Error::IOError`] with [`std::io::ErrorKind::UnexpectedEof`] if fewer than `n` bytes
    /// remain.
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let bytes = self.buf.get(self.pos..self.pos + n).ok_or_else(eof)?;
        self.pos += n;
        Ok(bytes)
    }

    /// Advances past `n` bytes without touching them.
    ///
    /// # Errors
    ///
    /// See [`Cursor::take`].
    pub fn skip(&mut self, n: usize) -> Result<(), Error> {
        self.take(n).map(|_| ())
    }

    /// Reads one byte.
    ///
    /// # Errors
    ///
    /// See [`Cursor::take`].
    pub fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
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

    /// Reads a property kind byte through [`Kind::unpack`].
    ///
    /// # Errors
    ///
    /// [`Error::InvalidPropertyTypePrimitive`] if the byte does not decode under the given
    /// numbering, or [`Error::IOError`] at the end of the slice.
    pub fn kind(&mut self, legacy: bool) -> Result<Kind, Error> {
        Kind::unpack(self.u8()?, legacy)
    }

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

fn eof() -> Error {
    Error::IOError(std::io::Error::from(std::io::ErrorKind::UnexpectedEof))
}

/// The fixed serialized width of `kind`, if it has one.
///
/// `None` for [`Kind::String`] (length-prefixed) and every complex kind (self-sized, or
/// recursive in [`Kind::Optional`]'s case). [`Kind::None`] is zero bytes wide.
#[must_use]
pub fn fixed_width(kind: Kind) -> Option<usize> {
    use Kind as K;
    match kind {
        K::None => Some(0),
        K::Bool | K::I8 | K::U8 | K::BitBool => Some(1),
        K::I16 | K::U16 => Some(2),
        K::I32 | K::U32 | K::F32 | K::Color | K::Hash | K::ObjectLink => Some(4),
        K::I64 | K::U64 | K::Vector2 | K::WadChunkLink => Some(8),
        K::Vector3 => Some(12),
        K::Vector4 => Some(16),
        K::Matrix44 => Some(64),
        K::String
        | K::Container
        | K::UnorderedContainer
        | K::Struct
        | K::Embedded
        | K::Optional
        | K::Map => None,
    }
}

/// Advances past one value of `kind` without decoding its contents.
///
/// Mirrors the client's `MetaValue_skipByType`: primitives by fixed width, strings by length
/// prefix, sized complex values by their stored byte size, [`Kind::Optional`] by recursing
/// into its zero-or-one element. A struct or embed whose class hash is `0` is a null pointer
/// and has no size field or body. `legacy` is only consulted where a skip has to decode a
/// kind byte, which is exactly the optional's element kind.
///
/// # Errors
///
/// [`Error::IOError`] if a skip distance runs past the end of the slice, or
/// [`Error::InvalidPropertyTypePrimitive`] if an optional's element kind byte does not decode.
pub fn skip_value(cur: &mut Cursor<'_>, kind: Kind, legacy: bool) -> Result<(), Error> {
    use Kind as K;
    if let Some(width) = fixed_width(kind) {
        return cur.skip(width);
    }
    match kind {
        K::String => {
            let len = cur.u16()? as usize;
            cur.skip(len)
        }
        K::Container | K::UnorderedContainer => {
            cur.skip(1)?; // item kind
            let size = cur.u32()? as usize;
            cur.skip(size)
        }
        K::Map => {
            cur.skip(2)?; // key and value kinds
            let size = cur.u32()? as usize;
            cur.skip(size)
        }
        K::Struct | K::Embedded => {
            let class = cur.u32()?;
            if class == 0 {
                return Ok(());
            }
            let size = cur.u32()? as usize;
            cur.skip(size)
        }
        K::Optional => {
            let item_kind = cur.kind(legacy)?;
            match cur.bool()? {
                true => skip_value(cur, item_kind, legacy),
                false => Ok(()),
            }
        }
        _ => unreachable!("every kind without a fixed width is matched above"),
    }
}

/// Reads the wire shape of a value of `kind` from its header bytes, without advancing.
///
/// Returns the same [`ValueShape`] the resolver's type rule uses, filled by the rules of
/// [`ValueShape::of`]: a container's or optional's item kind, a map's key and value kinds, an
/// embed's class. A pointer's ([`Kind::Struct`]'s) class is deliberately not recorded, so
/// nothing is read for one. The cursor is taken by value; the caller's position is untouched.
///
/// # Errors
///
/// [`Error::InvalidPropertyTypePrimitive`] if a header kind byte does not decode, or
/// [`Error::IOError`] at the end of the slice.
pub fn value_shape(mut cur: Cursor<'_>, kind: Kind, legacy: bool) -> Result<ValueShape, Error> {
    use Kind as K;
    let (item_kind, key_kind, class) = match kind {
        K::Container | K::UnorderedContainer | K::Optional => (Some(cur.kind(legacy)?), None, None),
        K::Map => {
            let key = cur.kind(legacy)?;
            let value = cur.kind(legacy)?;
            (Some(value), Some(key), None)
        }
        K::Embedded => (None, None, Some(cur.bin_hash()?)),
        _ => (None, None, None),
    };
    Ok(ValueShape {
        kind,
        item_kind,
        key_kind,
        class,
    })
}

/// Walks one value of `kind` by its counts, recording size discrepancies instead of raising.
///
/// This is the strict-by-counts walk of the stream's parse path: element and property counts
/// drive it, and a sized region's declared size is compared against what the walk consumed
/// only after the fact. On a mismatch a [`SizeDiscrepancy`] is recorded in `log` — with
/// offsets made absolute by adding `base_offset`, the stream offset of the slice's first
/// byte — and the cursor realigns to the declared next-offset, so the walk lands exactly
/// where a skip would have.
///
/// # Errors
///
/// [`Error::IOError`] if the walk runs past the end of the slice, or
/// [`Error::InvalidPropertyTypePrimitive`] if a kind byte does not decode.
pub fn walk_value(
    cur: &mut Cursor<'_>,
    kind: Kind,
    legacy: bool,
    base_offset: u64,
    log: &mut DiscrepancyLog,
) -> Result<(), Error> {
    use Kind as K;
    if let Some(width) = fixed_width(kind) {
        return cur.skip(width);
    }
    match kind {
        K::String => {
            let len = cur.u16()? as usize;
            cur.skip(len)
        }
        K::Container | K::UnorderedContainer => {
            let item_kind = cur.kind(legacy)?;
            sized_region(cur, base_offset, log, |cur, log| {
                let count = cur.u32()?;
                for _ in 0..count {
                    walk_value(cur, item_kind, legacy, base_offset, log)?;
                }
                Ok(())
            })
        }
        K::Map => {
            let key_kind = cur.kind(legacy)?;
            let value_kind = cur.kind(legacy)?;
            sized_region(cur, base_offset, log, |cur, log| {
                let count = cur.u32()?;
                for _ in 0..count {
                    walk_value(cur, key_kind, legacy, base_offset, log)?;
                    walk_value(cur, value_kind, legacy, base_offset, log)?;
                }
                Ok(())
            })
        }
        K::Struct | K::Embedded => {
            let class = cur.u32()?;
            if class == 0 {
                return Ok(());
            }
            sized_region(cur, base_offset, log, |cur, log| {
                let prop_count = cur.u16()?;
                for _ in 0..prop_count {
                    cur.skip(4)?; // name hash
                    let kind = cur.kind(legacy)?;
                    walk_value(cur, kind, legacy, base_offset, log)?;
                }
                Ok(())
            })
        }
        K::Optional => {
            let item_kind = cur.kind(legacy)?;
            match cur.bool()? {
                true => walk_value(cur, item_kind, legacy, base_offset, log),
                false => Ok(()),
            }
        }
        _ => unreachable!("every kind without a fixed width is matched above"),
    }
}

/// Reads a `u32` size field, runs `body`, and reconciles declared against consumed.
///
/// A disagreement is recorded, and the cursor lands on the declared next-offset either way
/// (clamped to the slice when the declared size overruns it).
fn sized_region(
    cur: &mut Cursor<'_>,
    base_offset: u64,
    log: &mut DiscrepancyLog,
    body: impl FnOnce(&mut Cursor<'_>, &mut DiscrepancyLog) -> Result<(), Error>,
) -> Result<(), Error> {
    let size_field = cur.position();
    let declared = cur.u32()? as usize;
    let start = cur.position();

    body(cur, log)?;

    let consumed = cur.position() - start;
    if consumed != declared {
        log.record(SizeDiscrepancy {
            offset: base_offset + size_field as u64,
            declared: declared as u64,
            consumed: consumed as u64,
        });
        cur.pos = usize::min(start + declared, cur.buf.len());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        concrete::values,
        property::values::{Embedded, UnorderedContainer},
        traits::PropertyExt as _,
        PropertyValueEnum,
    };
    use glam::{Mat4, Vec2, Vec3, Vec4};
    use ltk_primitives::Color;

    /// The value's body bytes, exactly as the eager writer lays them out.
    fn body(value: &PropertyValueEnum) -> Vec<u8> {
        let mut cursor = std::io::Cursor::new(Vec::new());
        value.to_writer(&mut cursor).expect("the value writes");
        cursor.into_inner()
    }

    /// One constructed value per kind, complex kinds included.
    fn one_of_each() -> Vec<PropertyValueEnum> {
        let object = values::Struct {
            class_hash: 0x1234u32.into(),
            properties: [
                (0x1111u32.into(), values::I32::new(42).into()),
                (0x2222u32.into(), values::String::from("hello").into()),
            ]
            .into_iter()
            .collect(),
            meta: Default::default(),
        };

        vec![
            values::None::default().into(),
            values::Bool::new(true).into(),
            values::BitBool::new(true).into(),
            values::I8::new(-1).into(),
            values::U8::new(1).into(),
            values::I16::new(-2).into(),
            values::U16::new(2).into(),
            values::I32::new(-3).into(),
            values::U32::new(3).into(),
            values::I64::new(-4).into(),
            values::U64::new(4).into(),
            values::F32::new(1.5).into(),
            values::Vector2::new(Vec2::ONE).into(),
            values::Vector3::new(Vec3::ONE).into(),
            values::Vector4::new(Vec4::ONE).into(),
            values::Matrix44::new(Mat4::IDENTITY).into(),
            values::Color::new(Color::new(1, 2, 3, 4)).into(),
            values::String::from("a string").into(),
            values::Hash::new(0xABCDu32).into(),
            values::WadChunkLink::new(0xDEAD_BEEFu64).into(),
            values::ObjectLink::new(0x4444u32).into(),
            object.clone().into(),
            // A null pointer: class hash 0, no size field, no body.
            values::Struct::default().into(),
            Embedded(object).into(),
            values::Container::from(vec![values::I32::new(1), values::I32::new(2)]).into(),
            values::Container::from(vec![
                values::String::from("one"),
                values::String::from("two"),
            ])
            .into(),
            UnorderedContainer(values::Container::from(vec![values::U8::new(9)])).into(),
            values::Optional::from(values::F32::new(2.5)).into(),
            values::Optional::empty(crate::PropertyKind::F32)
                .expect("F32 nests")
                .into(),
            values::Map::new(
                crate::PropertyKind::U32,
                crate::PropertyKind::String,
                vec![(
                    values::U32::new(7).into(),
                    values::String::from("seven").into(),
                )],
            )
            .expect("a valid map")
            .into(),
        ]
    }

    /// A skip's distance, a walk's distance, the written bytes and [`PropertyExt::size`] all
    /// have to agree, for every kind.
    #[test]
    fn skip_and_walk_distances_match_the_written_size() {
        for value in one_of_each() {
            let bytes = body(&value);
            assert_eq!(
                bytes.len(),
                value.size_no_header(),
                "{:?}: the writer and PropertyExt::size disagree",
                value.kind()
            );

            let mut cur = Cursor::new(&bytes);
            skip_value(&mut cur, value.kind(), false).expect("the value skips");
            assert_eq!(
                cur.position(),
                bytes.len(),
                "{:?}: skip distance is not the serialized size",
                value.kind()
            );

            let mut cur = Cursor::new(&bytes);
            let mut log = DiscrepancyLog::new();
            walk_value(&mut cur, value.kind(), false, 0, &mut log).expect("the value walks");
            assert_eq!(
                cur.position(),
                bytes.len(),
                "{:?}: walk distance is not the serialized size",
                value.kind()
            );
            assert!(log.is_empty(), "{:?}: a clean value logged", value.kind());
        }
    }

    #[test]
    fn shapes_come_from_the_header_bytes() {
        use crate::PropertyKind as K;
        for value in one_of_each() {
            let bytes = body(&value);
            let shape =
                value_shape(Cursor::new(&bytes), value.kind(), false).expect("the shape reads");
            assert_eq!(
                shape,
                crate::path::ValueShape::of(&value),
                "{:?}: the wire shape and ValueShape::of disagree",
                value.kind()
            );
        }

        // Spot-check the interesting ones.
        let map: PropertyValueEnum = values::Map::empty(K::Hash, K::I32).expect("valid").into();
        let shape = value_shape(Cursor::new(&body(&map)), K::Map, false).expect("reads");
        assert_eq!(shape.key_kind, Some(K::Hash));
        assert_eq!(shape.item_kind, Some(K::I32));
    }

    #[test]
    fn a_lying_size_is_recorded_and_realigned_not_raised() {
        let list: PropertyValueEnum =
            values::Container::from(vec![values::I32::new(1), values::I32::new(2)]).into();
        let mut bytes = body(&list);
        // The container's size field is bytes 1..5; the true body (count + two i32s) is 12.
        bytes[1..5].copy_from_slice(&20u32.to_le_bytes());
        bytes.extend_from_slice(&[0xFF; 8]); // the 8 padding bytes the lie claims

        let mut cur = Cursor::new(&bytes);
        let mut log = DiscrepancyLog::new();
        walk_value(
            &mut cur,
            crate::PropertyKind::Container,
            false,
            0x100,
            &mut log,
        )
        .expect("the walk records instead of failing");

        assert_eq!(
            log.retained(),
            [SizeDiscrepancy {
                offset: 0x101,
                declared: 20,
                consumed: 12,
            }]
        );
        // The walk lands where a skip would: on the declared next-offset.
        assert_eq!(cur.position(), bytes.len());
        let mut cur = Cursor::new(&bytes);
        skip_value(&mut cur, crate::PropertyKind::Container, false).expect("skips");
        assert_eq!(cur.position(), bytes.len());
    }

    #[test]
    fn leaf_codecs_decode_what_the_writer_wrote() {
        let value: PropertyValueEnum = values::String::from("héllo").into();
        let bytes = body(&value);
        let mut cur = Cursor::new(&bytes);
        assert_eq!(cur.str_u16().expect("valid UTF-8"), "héllo");
        assert_eq!(cur.remaining(), 0);

        let mut cur = Cursor::new(&bytes);
        assert!(matches!(cur.take(64), Err(Error::IOError(_))));
        assert_eq!(cur.position(), 0, "a failed take does not advance");
    }
}
