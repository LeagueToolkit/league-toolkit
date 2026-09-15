//! The delta write-back: a mounted base rewritten with whole-object edits.

use std::{collections::HashSet, io};

use byteorder::{WriteBytesExt as _, LE};
use indexmap::IndexMap;
use ltk_hash::{BinHash, WriteBytesExt as _};
use ltk_io_ext::WriterExt as _;

use crate::{
    stream::{BinStream, ObjectEntry},
    tree::write::WRITE_VERSION,
    BinKind, BinObject, Error,
};

/// Whole-object edits held against a mounted base.
///
/// Costs O(edited objects), not O(file). [`BinStream::write_patched`] writes the base with the
/// delta applied: every object the delta does not name is copied from the base byte for byte, and
/// only the objects it replaces or appends are encoded.
///
/// A delta holds whole objects. The edit itself is made on an owned object:
/// [`ObjectStream::read`](crate::stream::ObjectStream::read), then
/// [`BinObject::walk_mut`] or the object's fields.
///
/// # Examples
///
/// ```
/// use std::io::Cursor;
/// use ltk_meta::{property::values, Bin, BinDelta, BinObject, BinStream};
///
/// # let bin = Bin::builder()
/// #     .object(BinObject::builder(0x1111u32, 0x2222u32).property(0x3333u32, values::I32::new(1)).build())
/// #     .build();
/// # let mut bytes = Cursor::new(Vec::new());
/// # bin.to_writer(&mut bytes)?;
/// let mut stream = BinStream::mount(Cursor::new(bytes.into_inner()))?;
///
/// let mut object = stream.object(0x1111u32)?.expect("the object is in the bin").read()?;
/// object.insert(0x4444u32.into(), values::Bool::new(true));
///
/// let mut delta = BinDelta::new();
/// delta.replace(object);
///
/// let mut out = Vec::new();
/// stream.write_patched(&delta, &mut out)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BinDelta {
    /// Objects written in place of the base's, keyed by path hash.
    replaced: IndexMap<BinHash, BinObject>,
    /// Base objects dropped.
    removed: HashSet<BinHash>,
    /// Objects written after the base's, in the order appended, keyed by path hash.
    appended: IndexMap<BinHash, BinObject>,
    /// The dependency list written in place of the base's. `None` keeps the base's.
    dependencies: Option<Vec<String>>,
}

impl Default for BinDelta {
    fn default() -> Self {
        Self {
            replaced: IndexMap::new(),
            removed: HashSet::new(),
            appended: IndexMap::new(),
            dependencies: None,
        }
    }
}

impl BinDelta {
    /// An empty set of edits.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes `object` in place of the base object with the same path hash.
    ///
    /// Cancels a removal of that hash. Returns the replacement it displaces.
    pub fn replace(&mut self, object: BinObject) -> Option<BinObject> {
        self.removed.remove(&object.path_hash);
        self.replaced.insert(object.path_hash, object)
    }

    /// Drops the base object with `path_hash`.
    ///
    /// Cancels a replacement of that hash, and returns the replacement it cancels. An appended
    /// object is not a base object and stays appended. A hash the base does not hold makes
    /// [`BinStream::write_patched`] fail with [`Error::DeltaMissingObject`].
    pub fn remove(&mut self, path_hash: impl Into<BinHash>) -> Option<BinObject> {
        let path_hash = path_hash.into();
        self.removed.insert(path_hash);
        self.replaced.shift_remove(&path_hash)
    }

    /// Writes `object` after the base's objects, in the order appended.
    ///
    /// Returns the appended object with the same path hash it displaces. `object` takes the
    /// displaced object's position.
    pub fn append(&mut self, object: BinObject) -> Option<BinObject> {
        self.appended.insert(object.path_hash, object)
    }

    /// Writes `dependencies` in place of the base's dependency list.
    pub fn set_dependencies(&mut self, dependencies: impl IntoIterator<Item = impl Into<String>>) {
        self.dependencies = Some(dependencies.into_iter().map(Into::into).collect());
    }

    /// The replacement for the base object with `path_hash`, if the delta holds one.
    #[must_use]
    pub fn replacement(&self, path_hash: impl Into<BinHash>) -> Option<&BinObject> {
        self.replaced.get(&path_hash.into())
    }

    /// Whether the delta drops the base object with `path_hash`.
    #[must_use]
    pub fn is_removed(&self, path_hash: impl Into<BinHash>) -> bool {
        self.removed.contains(&path_hash.into())
    }

    /// The appended objects, in the order appended.
    #[must_use]
    pub fn appended(&self) -> indexmap::map::Values<'_, BinHash, BinObject> {
        self.appended.values()
    }

    /// The dependency list the delta writes, or `None` for the base's.
    #[must_use]
    pub fn dependencies(&self) -> Option<&[String]> {
        self.dependencies.as_deref()
    }

    /// Whether the delta contains no edits.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.replaced.is_empty()
            && self.removed.is_empty()
            && self.appended.is_empty()
            && self.dependencies.is_none()
    }
}

/// One object of the output, in output order.
enum Row<'d> {
    /// An untouched base object, copied from its byte range.
    Untouched(ObjectEntry),
    /// A replaced or appended object, encoded.
    Edited(&'d BinObject),
}

impl Row<'_> {
    fn class_hash(&self) -> BinHash {
        match self {
            Row::Untouched(entry) => entry.class_hash,
            Row::Edited(object) => object.class_hash,
        }
    }
}

impl<R: io::Read + io::Seek> BinStream<R> {
    /// Writes the base with `delta` applied.
    ///
    /// The header and class table are rebuilt for the final entry set. Every object the delta
    /// does not name is copied byte for byte from its [`ObjectEntry::byte_range`], without being
    /// read as values; replaced and appended objects are encoded by [`BinObject::to_writer`]. The
    /// entry order is the base's file order minus the removed objects, with each replaced object
    /// at its base position, then the appended objects in the order appended.
    ///
    /// The output uses version 3 and current property-kind numbering. Every base object is
    /// checked before output begins, including removed and replaced objects. The check reads
    /// kinds and verifies sizes without decoding leaf contents.
    ///
    /// # Errors
    ///
    /// [`Error::DeltaLegacyNumbering`] for a base using legacy numbering,
    /// [`Error::DeltaMissingObject`] for a replaced or removed hash the base does not hold,
    /// [`Error::DeltaDuplicateObject`] for an appended hash the output also holds, or an I/O
    /// error from the source or `out`. Invalid base sizes or kinds also return an error.
    /// Base validation and delta conflicts fail before any byte reaches `out`.
    pub fn write_patched<W: io::Write>(
        &mut self,
        delta: &BinDelta,
        out: &mut W,
    ) -> Result<(), Error> {
        if self.numbering().is_legacy() {
            return Err(Error::DeltaLegacyNumbering);
        }

        let toc = self.toc()?;
        if let Some(&missing) = delta
            .replaced
            .keys()
            .chain(&delta.removed)
            .find(|hash| toc.entry(**hash).is_none())
        {
            return Err(Error::DeltaMissingObject(missing));
        }
        if let Some(&duplicate) = delta
            .appended
            .keys()
            .find(|hash| toc.entry(**hash).is_some() && !delta.removed.contains(hash))
        {
            return Err(Error::DeltaDuplicateObject(duplicate));
        }

        let rows: Vec<Row<'_>> = toc
            .entries()
            .iter()
            .filter(|entry| !delta.removed.contains(&entry.path_hash))
            .map(|entry| match delta.replaced.get(&entry.path_hash) {
                Some(object) => Row::Edited(object),
                None => Row::Untouched(*entry),
            })
            .chain(delta.appended.values().map(Row::Edited))
            .collect();

        let mut objects = self.objects();
        while let Some(mut object) = objects.next()? {
            if object.view()?.numbering().is_legacy() {
                return Err(Error::DeltaLegacyNumbering);
            }
        }

        let dependencies = match &delta.dependencies {
            Some(dependencies) => dependencies.as_slice(),
            None => self.dependencies(),
        };
        out.write_all(&BinKind::Prop.magic())?;
        out.write_u32::<LE>(WRITE_VERSION)?;
        out.write_u32::<LE>(count(dependencies.len())?)?;
        for dependency in dependencies {
            out.write_len_prefixed_string::<LE, _>(dependency)?;
        }

        out.write_u32::<LE>(count(rows.len())?)?;
        for row in &rows {
            out.write_bin_hash::<LE>(row.class_hash())?;
        }

        let mut encoded = io::Cursor::new(Vec::new());
        for row in rows {
            match row {
                Row::Untouched(entry) => self.copy_range(entry.byte_range(), out)?,
                Row::Edited(object) => {
                    encoded.get_mut().clear();
                    encoded.set_position(0);
                    object.to_writer(&mut encoded)?;
                    out.write_all(encoded.get_ref())?;
                }
            }
        }

        Ok(())
    }
}

/// `len` as the `u32` count the file stores.
fn count(len: usize) -> Result<u32, Error> {
    u32::try_from(len).map_err(|_| {
        Error::IOError(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{len} entries do not fit a u32 count"),
        ))
    })
}

#[cfg(test)]
mod tests;
