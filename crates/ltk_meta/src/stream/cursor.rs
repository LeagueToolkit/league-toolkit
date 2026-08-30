use std::{io, ops::Range};

use ltk_hash::BinHash;

use crate::{
    property::NoMeta,
    stream::{BinStream, ObjectEntry, ObjectView},
    BinObject, Error,
};

/// Streaming cursor over the object table.
///
/// Not a `std` iterator: each yielded [`ObjectStream`] borrows the reader, so the borrow
/// checker enforces one open object at a time. For plain descriptors through a real iterator,
/// use [`Entries`].
#[must_use = "cursors are lazy and read nothing until advanced"]
#[derive(Debug)]
pub struct Objects<'a, R: io::Read + io::Seek, M = NoMeta> {
    stream: &'a mut BinStream<R, M>,
    index: usize,
}

impl<'a, R: io::Read + io::Seek, M: Default> Objects<'a, R, M> {
    pub(crate) fn new(stream: &'a mut BinStream<R, M>) -> Self {
        Self { stream, index: 0 }
    }

    /// Advances to the next object, skipping whatever the previous one did not consume.
    ///
    /// Reads the 8-byte object header (`size`, `path_hash`); the class hash comes from the
    /// table read at mount. Feeds the TOC as it goes, and serves rows the TOC already holds
    /// without touching the reader. After an error the cursor is exhausted.
    ///
    /// # Errors
    ///
    /// An I/O error from the source.
    #[expect(
        clippy::should_implement_trait,
        reason = "a lending cursor: the yielded item borrows the reader, which `Iterator` cannot express"
    )]
    pub fn next(&mut self) -> Result<Option<ObjectStream<'_, R, M>>, Error> {
        if self.index == self.stream.class_hashes().len() {
            return Ok(None);
        }

        let entry = match self.stream.toc_row(self.index) {
            Some(&entry) => entry,
            None => match self.stream.harvest(self.index) {
                Ok(entry) => entry,
                Err(error) => {
                    self.index = self.stream.class_hashes().len();
                    return Err(error);
                }
            },
        };
        self.index += 1;

        Ok(Some(ObjectStream::new(self.stream, entry)))
    }
}

/// `std` iterator of plain [`ObjectEntry`] descriptors — [`Objects`] without descent.
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[derive(Debug)]
pub struct Entries<'a, R: io::Read + io::Seek, M = NoMeta> {
    objects: Objects<'a, R, M>,
}

impl<'a, R: io::Read + io::Seek, M: Default> Entries<'a, R, M> {
    pub(crate) fn new(stream: &'a mut BinStream<R, M>) -> Self {
        Self {
            objects: Objects::new(stream),
        }
    }
}

impl<R: io::Read + io::Seek, M: Default> Iterator for Entries<'_, R, M> {
    type Item = Result<ObjectEntry, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.objects.next() {
            Ok(Some(object)) => Some(Ok(object.entry())),
            Ok(None) => None,
            Err(error) => Some(Err(error)),
        }
    }
}

/// A view of one object positioned in the stream.
///
/// Dropping it without descending costs nothing; the parent cursor skips by size. Everything
/// here except [`ObjectStream::property_count`] is served from the already-harvested
/// [`ObjectEntry`] without touching the reader.
#[derive(Debug)]
pub struct ObjectStream<'a, R: io::Read + io::Seek, M = NoMeta> {
    stream: &'a mut BinStream<R, M>,
    entry: ObjectEntry,
    property_count: Option<u16>,
}

impl<'a, R: io::Read + io::Seek, M: Default> ObjectStream<'a, R, M> {
    pub(crate) fn new(stream: &'a mut BinStream<R, M>, entry: ObjectEntry) -> Self {
        Self {
            stream,
            entry,
            property_count: None,
        }
    }

    /// The object's path hash.
    #[must_use]
    pub fn path_hash(&self) -> BinHash {
        self.entry.path_hash
    }

    /// The object's class hash.
    #[must_use]
    pub fn class_hash(&self) -> BinHash {
        self.entry.class_hash
    }

    /// The object's row of the table of contents.
    #[must_use]
    pub fn entry(&self) -> ObjectEntry {
        self.entry
    }

    /// The object's raw byte range in the stream (`size` field included), as a byte-exact
    /// copy of the object needs.
    #[must_use]
    pub fn byte_range(&self) -> Range<u64> {
        self.entry.byte_range()
    }

    /// Number of properties, read from the object header on first use.
    ///
    /// # Errors
    ///
    /// An I/O error from the source.
    pub fn property_count(&mut self) -> Result<u16, Error> {
        if let Some(count) = self.property_count {
            return Ok(count);
        }
        // The count sits after the object's `u32 size` and `u32 path_hash`.
        let count = self.stream.read_property_count(self.entry.offset + 8)?;
        self.property_count = Some(count);
        Ok(count)
    }

    /// Buffers the object's declared byte range and returns a zero-copy view over it.
    ///
    /// One read into the handle's reused buffer, then everything inside the object —
    /// iteration, random access, descent to any depth — happens in memory. Nothing decodes
    /// until it is touched.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidSize`] if the object's declared size disagrees with what its property
    /// counts consume, [`Error::InvalidPropertyTypePrimitive`] if a kind byte decodes under
    /// neither numbering, or an I/O error from the source.
    pub fn view(&mut self) -> Result<ObjectView<'_, M>, Error> {
        let entry = self.entry;
        ObjectView::new(entry, self.stream.view_object(entry)?)
    }

    /// Parses the whole object into an eager [`BinObject`].
    ///
    /// `read`, not `parse`: it does I/O, and the crate's vocabulary is `from_reader` /
    /// [`ReadProperty`](crate::traits::ReadProperty). Equivalent to [`ObjectStream::view`] plus
    /// an owned decode through the layout core, and the right call for an editor, which wants an
    /// object it owns outright rather than the shared `Arc`
    /// [`BinStream::cached_object`](crate::stream::BinStream::cached_object) hands out.
    ///
    /// # Errors
    ///
    /// The same as [`ObjectStream::view`], plus whatever the value model raises for a container
    /// it refuses: [`Error::InvalidNesting`], [`Error::InvalidKeyType`],
    /// [`Error::MismatchedContainerTypes`].
    pub fn read(&mut self) -> Result<BinObject<M>, Error> {
        self.stream.read_object(self.entry)
    }
}
