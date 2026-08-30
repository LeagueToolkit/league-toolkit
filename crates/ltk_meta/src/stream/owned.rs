//! The owned renderer over the layout core: bytes in, [`PropertyValueEnum`] out.
//!
//! One of the two surfaces section 9 of the streaming design puts over
//! [`layout`](crate::stream::layout) — the other being the borrowed
//! [views](crate::stream::ObjectView). Everything the crate parses lands here, so the eager
//! [`Bin::from_reader`](crate::Bin::from_reader) and a [`ValueView`](crate::stream::ValueView)
//! can never disagree about the same bytes.
//!
//! Nothing takes a numbering flag: a [`Cursor`] carries the one its bytes were written under,
//! so it cannot be forwarded wrongly or forgotten.
//!
//! Complex values are built through the value model's checked constructors, which is what keeps
//! [`Error::InvalidNesting`], [`Error::InvalidKeyType`] and
//! [`Error::MismatchedContainerTypes`] the value model's business rather than the stream's.

use std::io;

use byteorder::{ReadBytesExt as _, LE};
use indexmap::IndexMap;
use ltk_hash::BinHash;

use crate::{
    property::{values, Kind},
    stream::layout::{Cursor, Numbering},
    BinObject, Error, PropertyValueEnum,
};

/// How many bytes to reach for first when a value's extent is not known up front.
const PROBE: usize = 256;

/// The largest single allocation a fill will make before it has seen the bytes exist.
///
/// A declared size is attacker-controlled, so a short source must not be able to ask for a
/// gigabyte of zeroed memory. Growing in chunks caps the waste at one chunk.
const CHUNK: usize = 64 * 1024;

/// Decodes one whole object — its `u32 size` field included — from `cur`.
///
/// # Errors
///
/// [`Error::InvalidSize`] if the declared size disagrees with what the property counts
/// consumed, or whatever [`read_value`] raises for one of the properties.
pub(crate) fn read_object<M: Default>(
    cur: &mut Cursor<'_>,
    class_hash: BinHash,
) -> Result<BinObject<M>, Error> {
    cur.sized_region(|cur| read_object_body(cur, class_hash))
}

/// Decodes an object's body, for a caller that has already read its `u32 size` field.
fn read_object_body<M: Default>(
    cur: &mut Cursor<'_>,
    class_hash: BinHash,
) -> Result<BinObject<M>, Error> {
    let path_hash = cur.bin_hash()?;
    let count = cur.u16()?;
    Ok(BinObject {
        path_hash,
        class_hash,
        properties: read_properties(cur, count)?,
    })
}

/// Decodes `count` `name_hash`/`kind`/`value` triples.
///
/// # Errors
///
/// See [`read_value`].
pub(crate) fn read_properties<M: Default>(
    cur: &mut Cursor<'_>,
    count: u16,
) -> Result<IndexMap<BinHash, PropertyValueEnum<M>>, Error> {
    let mut properties = IndexMap::with_capacity(count as usize);
    for _ in 0..count {
        let name_hash = cur.bin_hash()?;
        let kind = cur.kind()?;
        properties.insert(name_hash, read_value(cur, kind)?);
    }
    Ok(properties)
}

/// Decodes one value of `kind`, and the whole subtree under it.
///
/// # Errors
///
/// [`Error::InvalidSize`] if a declared size disagrees with what the counts consumed,
/// [`Error::InvalidNesting`] / [`Error::InvalidKeyType`] for a container the value model
/// refuses, [`Error::InvalidPropertyTypePrimitive`] for a kind byte that does not decode,
/// [`Error::Utf8Error`] for a string that is not UTF-8, or [`Error::IOError`] at the end of
/// the slice.
pub(crate) fn read_value<M: Default>(
    cur: &mut Cursor<'_>,
    kind: Kind,
) -> Result<PropertyValueEnum<M>, Error> {
    use Kind as K;
    Ok(match kind {
        K::None => values::None::default().into(),
        K::Bool => values::Bool::new(cur.bool()?).into(),
        K::BitBool => values::BitBool::new(cur.bool()?).into(),
        K::I8 => values::I8::new(cur.i8()?).into(),
        K::U8 => values::U8::new(cur.u8()?).into(),
        K::I16 => values::I16::new(cur.i16()?).into(),
        K::U16 => values::U16::new(cur.u16()?).into(),
        K::I32 => values::I32::new(cur.i32()?).into(),
        K::U32 => values::U32::new(cur.u32()?).into(),
        K::I64 => values::I64::new(cur.i64()?).into(),
        K::U64 => values::U64::new(cur.u64()?).into(),
        K::F32 => values::F32::new(cur.f32()?).into(),
        K::Vector2 => values::Vector2::new(cur.vec2()?).into(),
        K::Vector3 => values::Vector3::new(cur.vec3()?).into(),
        K::Vector4 => values::Vector4::new(cur.vec4()?).into(),
        K::Matrix44 => values::Matrix44::new(cur.mat4_row_major()?).into(),
        K::Color => values::Color::new(cur.color_u8()?).into(),
        K::String => read_string(cur)?.into(),
        K::Hash => values::Hash::new(cur.bin_hash()?).into(),
        K::WadChunkLink => values::WadChunkLink::new(cur.wad_hash()?).into(),
        K::ObjectLink => values::ObjectLink::new(cur.bin_hash()?).into(),
        K::Container => read_container(cur)?.into(),
        K::UnorderedContainer => values::UnorderedContainer(read_container(cur)?).into(),
        K::Optional => read_optional(cur)?.into(),
        K::Map => read_map(cur)?.into(),
        K::Struct => read_struct(cur)?.into(),
        K::Embedded => values::Embedded(read_struct(cur)?).into(),
    })
}

/// Decodes a [`Kind::String`] body.
///
/// # Errors
///
/// [`Error::Utf8Error`] if the bytes are not UTF-8, or [`Error::IOError`] at the end of the
/// slice.
pub(crate) fn read_string<M: Default>(cur: &mut Cursor<'_>) -> Result<values::String<M>, Error> {
    Ok(values::String::new(cur.str_u16()?.to_owned()))
}

/// Decodes a [`Kind::Container`] body.
///
/// # Errors
///
/// See [`read_value`].
pub(crate) fn read_container<M: Default>(
    cur: &mut Cursor<'_>,
) -> Result<values::Container<M>, Error> {
    // Checked before the body is walked, not after: a nested item kind would make every byte
    // that follows mean something else.
    let item_kind = cur.kind()?;
    if item_kind.is_container() {
        return Err(Error::InvalidNesting(item_kind));
    }

    let items = cur.sized_region(|cur| {
        let count = cur.u32()?;
        let mut items = Vec::with_capacity(count as usize);
        for _ in 0..count {
            items.push(read_value(cur, item_kind)?);
        }
        Ok(items)
    })?;

    values::Container::new(item_kind, items)
}

/// Decodes a [`Kind::Map`] body.
///
/// # Errors
///
/// See [`read_value`].
pub(crate) fn read_map<M: Default>(cur: &mut Cursor<'_>) -> Result<values::Map<M>, Error> {
    let key_kind = cur.kind()?;
    if !key_kind.is_valid_map_key() {
        return Err(Error::InvalidKeyType(key_kind));
    }
    let value_kind = cur.kind()?;
    if value_kind.is_container() {
        return Err(Error::InvalidNesting(value_kind));
    }

    let entries = cur.sized_region(|cur| {
        let count = cur.u32()?;
        let mut entries = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let key = read_value(cur, key_kind)?;
            let value = read_value(cur, value_kind)?;
            entries.push((key, value));
        }
        Ok(entries)
    })?;

    values::Map::new(key_kind, value_kind, entries)
}

/// Decodes a [`Kind::Optional`] body.
///
/// # Errors
///
/// See [`read_value`].
pub(crate) fn read_optional<M: Default>(
    cur: &mut Cursor<'_>,
) -> Result<values::Optional<M>, Error> {
    let item_kind = cur.kind()?;
    if item_kind.is_container() {
        return Err(Error::InvalidNesting(item_kind));
    }

    let value = match cur.bool()? {
        true => Some(read_value(cur, item_kind)?),
        false => None,
    };

    values::Optional::new(item_kind, value)
}

/// Decodes a [`Kind::Struct`] or [`Kind::Embedded`] body.
///
/// A class hash of `0` is a null pointer: no size field, no body.
///
/// # Errors
///
/// See [`read_value`].
pub(crate) fn read_struct<M: Default>(cur: &mut Cursor<'_>) -> Result<values::Struct<M>, Error> {
    let class_hash = cur.bin_hash()?;
    if *class_hash == 0 {
        return Ok(values::Struct {
            class_hash,
            ..Default::default()
        });
    }

    cur.sized_region(|cur| {
        let count = cur.u16()?;
        Ok(values::Struct {
            class_hash,
            properties: read_properties(cur, count)?,
            meta: M::default(),
        })
    })
}

/// Decodes one whole object from `reader`, which must be at the object's `u32 size` field.
///
/// The size field bounds the object, so the bytes are taken in one pass with no probing.
///
/// # Errors
///
/// [`Error::InvalidSize`] if the object's properties end before the declared size does, which
/// is also how a truncated source usually surfaces; [`Error::IOError`] if the counts run past
/// what the source had; otherwise see [`read_object`].
pub(crate) fn read_object_from<M: Default, R: io::Read + io::Seek + ?Sized>(
    reader: &mut R,
    class_hash: BinHash,
    numbering: Numbering,
) -> Result<BinObject<M>, Error> {
    let declared = reader.read_u32::<LE>()? as usize;

    let mut body = Vec::new();
    fill_to(reader, &mut body, declared)?;

    let mut cur = Cursor::new(&body, numbering);
    let object = read_object_body(&mut cur, class_hash)?;

    let consumed = cur.position();
    match consumed == declared {
        true => Ok(object),
        false => Err(Error::InvalidSize(declared as u64, consumed as u64)),
    }
}

/// Decodes one value of `kind` from `reader`, leaving it immediately past that value.
///
/// Only [`layout`](crate::stream::layout) knows how far a value reaches, so the bytes are
/// gathered by growing a buffer until [`Cursor::walk_value`] can cross it, and the reader is
/// then wound back over whatever the probe over-read. That is what keeps this — the entry
/// point every [`ReadProperty`](crate::traits::ReadProperty) impl for a self-sized value goes
/// through — from being a second set of extent rules.
///
/// # Errors
///
/// See [`read_value`]. [`Error::IOError`] if the source runs out before the value does.
pub(crate) fn read_value_from<M: Default, R: io::Read + io::Seek + ?Sized>(
    reader: &mut R,
    kind: Kind,
    numbering: Numbering,
) -> Result<PropertyValueEnum<M>, Error> {
    read_from(reader, kind, numbering, |cur| read_value(cur, kind))
}

/// Gathers one value of `kind` from `reader` and hands its exact bytes to `decode`.
///
/// See [`read_value_from`]; this is the same bridge for a caller that wants one concrete value
/// type back rather than a [`PropertyValueEnum`].
///
/// # Errors
///
/// See [`read_value_from`].
pub(crate) fn read_from<T, R, F>(
    reader: &mut R,
    kind: Kind,
    numbering: Numbering,
    decode: F,
) -> Result<T, Error>
where
    R: io::Read + io::Seek + ?Sized,
    F: FnOnce(&mut Cursor<'_>) -> Result<T, Error>,
{
    let mut buf = Vec::new();
    let extent = fill_value(reader, kind, numbering, &mut buf)?;

    // A fixed-width kind is read to the byte, so most values need no winding at all.
    let over_read = buf.len() - extent;
    if over_read > 0 {
        reader.seek(io::SeekFrom::Current(-(over_read as i64)))?;
    }

    decode(&mut Cursor::new(&buf[..extent], numbering))
}

/// Grows `buf` from `reader` until one value of `kind` fits, returning that value's length.
fn fill_value<R: io::Read + io::Seek + ?Sized>(
    reader: &mut R,
    kind: Kind,
    numbering: Numbering,
    buf: &mut Vec<u8>,
) -> Result<usize, Error> {
    // A fixed-width kind needs exactly its width and never probes; everything else guesses.
    let mut want = kind.fixed_width().unwrap_or(PROBE);
    loop {
        fill_to(reader, buf, want)?;
        let exhausted = buf.len() < want;

        let mut cur = Cursor::new(buf.as_slice(), numbering);
        match cur.walk_value(kind) {
            Ok(()) => return Ok(cur.position()),
            // The probe was too small — unless the source had nothing more to give, in which
            // case the value really is truncated and the walk's error is the right one.
            Err(error @ Error::IOError(_)) => match exhausted {
                true => return Err(error),
                false => want = want.saturating_mul(2),
            },
            Err(error) => return Err(error),
        }
    }
}

/// Reads from `reader` until `buf` holds `want` bytes, or the source is exhausted.
///
/// `want` is never trusted as an allocation size: the buffer grows a chunk at a time, so a
/// declared size far larger than the file costs nothing to refuse.
pub(crate) fn fill_to<R: io::Read + ?Sized>(
    reader: &mut R,
    buf: &mut Vec<u8>,
    want: usize,
) -> Result<(), Error> {
    while buf.len() < want {
        let start = buf.len();
        let end = start + (want - start).min(CHUNK);
        buf.resize(end, 0);

        let mut filled = start;
        while filled < end {
            match reader.read(&mut buf[filled..end]) {
                Ok(0) => {
                    buf.truncate(filled);
                    return Ok(());
                }
                Ok(read) => filled += read,
                // What `read_exact` does with one, and what the readers this replaced did.
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => {
                    buf.truncate(filled);
                    return Err(error.into());
                }
            }
        }
    }
    Ok(())
}
