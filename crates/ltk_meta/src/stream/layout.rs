//! How property values lay out in bytes, and one cursor that knows it.
//!
//! Every layer that touches value bytes — the streaming sweep, the buffered object views, the
//! owned decode — moves through a [`Cursor`], so no two of them can disagree about where a
//! value starts, how far it runs, or what its bytes mean. The rules mirror the client's
//! `MetaValue_skipByType`: primitives have a fixed width, strings a `u16` length prefix,
//! complex values carry their byte size ahead of their body, [`Kind::Optional`] is as wide as
//! the zero-or-one element it holds, and [`Kind::BitBool`] is one byte.
//!
//! A cursor carries the [`Numbering`] its bytes were written under, so nothing downstream has
//! to thread a flag through every call — and a slice can never be read under the wrong
//! numbering by accident, because the two travel together.
//!
//! Two ways past a value, and the difference is the whole strictness model:
//!
//! - [`Cursor::skip_value`] trusts the declared size and moves by it, decoding nothing. This is
//!   what the sweep does to the bodies it will not read.
//! - [`Cursor::walk_value`] is driven by the counts, exactly as the client's parser is, and
//!   compares a sized region's declared size against what those counts consumed. A
//!   disagreement means the file's skip path and parse path describe different bytes, which is
//!   [`Error::InvalidSize`].
//!
//! Nothing here allocates, and nothing decodes a value's contents to move past it. Decoding
//! happens only when a leaf codec is asked for.

mod codec;

#[cfg(test)]
mod tests;

use crate::{path::ValueShape, property::Kind, Error};

/// Which property-kind numbering a cursor's bytes were written under.
///
/// `WadChunkLink` was added in the middle of the kind enum, so files written before it exist
/// number every complex kind lower than a file written after it does. [`Kind::unpack`] is where
/// the difference is applied; this is how a cursor remembers which side of it the bytes are on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Numbering {
    /// `WadChunkLink` exists, and kind bytes mean what [`Kind`] says they mean.
    #[default]
    Current,
    /// Written before `WadChunkLink` existed, so the complex kinds sit lower.
    Legacy,
}

impl Numbering {
    /// The numbering a `legacy` flag names.
    ///
    /// The public reader surface — [`ReadProperty`](crate::traits::ReadProperty),
    /// [`Kind::unpack`] — still spells the question as a `bool`, so this is where that spelling
    /// is converted into the one the cursor carries.
    #[must_use]
    pub fn from_legacy(legacy: bool) -> Self {
        match legacy {
            true => Self::Legacy,
            false => Self::Current,
        }
    }

    /// Whether this is [`Numbering::Legacy`], in the form [`Kind::unpack`] takes.
    #[must_use]
    pub fn is_legacy(self) -> bool {
        matches!(self, Self::Legacy)
    }
}

/// A reading position over a byte slice, and the numbering those bytes use.
///
/// The slice is never copied, so a cursor is [`Copy`] and forking one to read ahead costs
/// nothing. Reading past the end fails with [`Error::IOError`] carrying
/// [`std::io::ErrorKind::UnexpectedEof`].
#[derive(Debug, Clone, Copy)]
pub struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
    numbering: Numbering,
}

impl<'a> Cursor<'a> {
    /// A cursor at the start of `buf`, reading it under `numbering`.
    #[must_use]
    pub fn new(buf: &'a [u8], numbering: Numbering) -> Self {
        Self {
            buf,
            pos: 0,
            numbering,
        }
    }

    /// The numbering this cursor reads kind bytes under.
    #[must_use]
    pub fn numbering(&self) -> Numbering {
        self.numbering
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

    /// The bytes ahead of the cursor, without moving it.
    #[must_use]
    pub fn rest(&self) -> &'a [u8] {
        &self.buf[self.pos..]
    }

    /// The next `n` bytes, advancing past them.
    ///
    /// # Errors
    ///
    /// [`Error::IOError`] with [`std::io::ErrorKind::UnexpectedEof`] if fewer than `n` bytes
    /// remain.
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.pos.checked_add(n).ok_or_else(eof)?;
        let bytes = self.buf.get(self.pos..end).ok_or_else(eof)?;
        self.pos = end;
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

    // ── moving past a value ─────────────────────────────────────────────────

    /// Advances past one value of `kind` without decoding its contents.
    ///
    /// A struct or embed whose class hash is `0` is a null pointer: no size field, no body.
    ///
    /// # Errors
    ///
    /// [`Error::IOError`] if a skip distance runs past the end of the slice, or
    /// [`Error::InvalidPropertyTypePrimitive`] if an optional's element kind byte does not
    /// decode — the one place a skip has to read a kind at all.
    pub fn skip_value(&mut self, kind: Kind) -> Result<(), Error> {
        use Kind as K;
        if let Some(width) = kind.fixed_width() {
            return self.skip(width);
        }
        match kind {
            K::String => {
                let len = self.u16()? as usize;
                self.skip(len)
            }
            K::Container | K::UnorderedContainer => {
                self.skip(1)?; // item kind
                let size = self.u32()? as usize;
                self.skip(size)
            }
            K::Map => {
                self.skip(2)?; // key and value kinds
                let size = self.u32()? as usize;
                self.skip(size)
            }
            K::Struct | K::Embedded => {
                if self.u32()? == 0 {
                    return Ok(());
                }
                let size = self.u32()? as usize;
                self.skip(size)
            }
            K::Optional => {
                let item_kind = self.kind()?;
                match self.bool()? {
                    true => self.skip_value(item_kind),
                    false => Ok(()),
                }
            }
            _ => unreachable!("every kind without a fixed width is matched above"),
        }
    }

    /// The bytes one value of `kind` occupies, advancing past it.
    ///
    /// What the views hand out as a property's or an item's raw bytes: exactly what the writer
    /// emits for that value, and exactly what
    /// [`PropertyExt::size_no_header`](crate::traits::PropertyExt::size_no_header) measures.
    ///
    /// # Errors
    ///
    /// See [`Cursor::skip_value`].
    pub fn take_value(&mut self, kind: Kind) -> Result<&'a [u8], Error> {
        let start = self.pos;
        self.skip_value(kind)?;
        Ok(&self.buf[start..self.pos])
    }

    // ── reading a value's header ────────────────────────────────────────────

    /// The shape a value of `kind` declares, read from the header bytes ahead of its body.
    ///
    /// Returns the same [`ValueShape`] the resolver's type rule uses, filled by the rules of
    /// [`ValueShape::of`]: a container's or option's item kind, a map's key and value kinds, an
    /// embed's class. A pointer's class is deliberately not recorded, so nothing is read for
    /// one. The cursor does not move.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidPropertyTypePrimitive`] if a header kind byte does not decode, or
    /// [`Error::IOError`] if the bytes end inside the header.
    pub fn value_shape(&self, kind: Kind) -> Result<ValueShape, Error> {
        use Kind as K;
        let mut ahead = *self;
        let (item_kind, key_kind, class) = match kind {
            K::Container | K::UnorderedContainer | K::Optional => (Some(ahead.kind()?), None, None),
            K::Map => {
                let key = ahead.kind()?;
                let value = ahead.kind()?;
                (Some(value), Some(key), None)
            }
            K::Embedded => (None, None, Some(ahead.bin_hash()?)),
            _ => (None, None, None),
        };

        Ok(ValueShape {
            kind,
            item_kind,
            key_kind,
            class,
        })
    }

    // ── walking a value by its counts ───────────────────────────────────────

    /// Walks one value of `kind` by its counts, verifying declared sizes along the way.
    ///
    /// This is the parse path's traversal: element and property counts drive it, exactly as
    /// they drive the client's parser. A sized region's declared size is compared against what
    /// the walk consumed after the fact — the two describing different bytes means the file is
    /// internally inconsistent, so the walk fails rather than guessing which one to believe.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidSize`] if a declared size disagrees with what the counts consumed,
    /// [`Error::IOError`] if the walk runs past the end of the slice, or
    /// [`Error::InvalidPropertyTypePrimitive`] if a kind byte does not decode.
    pub fn walk_value(&mut self, kind: Kind) -> Result<(), Error> {
        use Kind as K;
        if let Some(width) = kind.fixed_width() {
            return self.skip(width);
        }
        match kind {
            K::String => {
                let len = self.u16()? as usize;
                self.skip(len)
            }
            K::Container | K::UnorderedContainer => {
                let item_kind = self.kind()?;
                self.sized_region(|cur| {
                    let count = cur.u32()?;
                    for _ in 0..count {
                        cur.walk_value(item_kind)?;
                    }
                    Ok(())
                })
            }
            K::Map => {
                let key_kind = self.kind()?;
                let value_kind = self.kind()?;
                self.sized_region(|cur| {
                    let count = cur.u32()?;
                    for _ in 0..count {
                        cur.walk_value(key_kind)?;
                        cur.walk_value(value_kind)?;
                    }
                    Ok(())
                })
            }
            K::Struct | K::Embedded => {
                if self.u32()? == 0 {
                    return Ok(());
                }
                self.sized_region(|cur| {
                    let count = cur.u16()?;
                    cur.walk_properties(count)
                })
            }
            K::Optional => {
                let item_kind = self.kind()?;
                match self.bool()? {
                    true => self.walk_value(item_kind),
                    false => Ok(()),
                }
            }
            _ => unreachable!("every kind without a fixed width is matched above"),
        }
    }

    /// Walks one whole object — its `u32 size` field included — by the same rules.
    ///
    /// An object's sized region is its path hash, its property count, and that many properties.
    /// This is the walk a buffered object gets as it lands: it settles the numbering latch and
    /// proves the declared size before anything reads inside it.
    ///
    /// # Errors
    ///
    /// The same as [`Cursor::walk_value`].
    pub fn walk_object(&mut self) -> Result<(), Error> {
        self.sized_region(|cur| {
            cur.skip(4)?; // path hash
            let count = cur.u16()?;
            cur.walk_properties(count)
        })
    }

    /// Walks `count` `name_hash`/`kind`/`value` triples.
    fn walk_properties(&mut self, count: u16) -> Result<(), Error> {
        for _ in 0..count {
            self.skip(4)?; // name hash
            let kind = self.kind()?;
            self.walk_value(kind)?;
        }
        Ok(())
    }

    /// Reads a `u32` size field, runs `body`, and checks declared against consumed.
    ///
    /// The one place [`Error::InvalidSize`] comes from, which is why both renderers over this
    /// module raise it on exactly the same inputs.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidSize`] if `body` did not consume exactly the declared size, plus
    /// whatever `body` itself raises.
    pub fn sized_region<T>(
        &mut self,
        body: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let declared = self.u32()? as usize;
        let start = self.pos;

        let value = body(self)?;

        let consumed = self.pos - start;
        match consumed == declared {
            true => Ok(value),
            false => Err(Error::InvalidSize(declared as u64, consumed as u64)),
        }
    }
}

pub(crate) fn eof() -> Error {
    Error::IOError(std::io::Error::from(std::io::ErrorKind::UnexpectedEof))
}
