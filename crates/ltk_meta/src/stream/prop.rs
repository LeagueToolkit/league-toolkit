use std::{
    io::{self, Seek as _},
    marker::PhantomData,
};

use byteorder::{ReadBytesExt as _, LE};
use ltk_hash::{BinHash, ReadBytesExt as _};
use ltk_io_ext::ReaderExt as _;

use crate::{
    property::NoMeta,
    stream::{BinToc, Entries, ObjectEntry, ObjectStream, Objects},
    BinKind, Error,
};

/// A mounted `PROP` stream: the header is parsed, the object table is read on demand.
///
/// Owns its source and buffers internally ([`io::BufReader`] + `seek_relative`, so the sweep's
/// short hops stay inside the buffer). Hand it the bare [`File`](std::fs::File); pre-wrapping
/// in a `BufReader` only double-buffers.
///
/// `M` is the same property-meta parameter the eager types carry; the
/// [`concrete`](crate::concrete) alias pins it to [`NoMeta`] at the mount call.
///
/// # Examples
///
/// ```no_run
/// use std::fs::File;
/// use ltk_meta::concrete::BinStream;
///
/// let mut stream = BinStream::mount(File::open("data.bin")?)?;
/// println!("version {}, {} objects", stream.version(), stream.class_hashes().len());
///
/// for entry in stream.entries() {
///     println!("{:08x}", entry?.path_hash);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct BinStream<R: io::Read + io::Seek, M = NoMeta> {
    reader: io::BufReader<R>,
    version: u32,
    dependencies: Vec<String>,
    class_hashes: Vec<BinHash>,
    /// Absolute offset of the first object's `u32` size field.
    objects_start: u64,
    /// Grows as sweeps discover rows; complete once it holds every object.
    toc: BinToc,
    /// The legacy kind-numbering latch. Mounting starts in current numbering; the latch that
    /// flips this mid-sweep arrives with value parsing, which threads it into
    /// [`Kind::unpack`](crate::PropertyKind::unpack) as the `legacy` argument.
    #[expect(dead_code, reason = "read once the value-parsing layer lands")]
    legacy: bool,
    meta: PhantomData<fn() -> M>,
}

impl<R: io::Read + io::Seek, M: Default> BinStream<R, M> {
    /// Mounts a `PROP` stream, reading the header, dependencies and class-hash table.
    ///
    /// Reads sequentially to the start of the object bodies and stops; nothing past the
    /// header is touched until something asks for it.
    ///
    /// # Errors
    ///
    /// [`Error::UnexpectedBinKind`] for a `PTCH` stream, [`Error::InvalidFileSignature`] if
    /// the magic belongs to neither kind, [`Error::InvalidFileVersion`] for a version this
    /// crate does not read, or an I/O error from the source.
    pub fn mount(source: R) -> Result<Self, Error> {
        let mut reader = io::BufReader::new(source);

        match BinKind::from_magic_u32(reader.read_u32::<LE>()?) {
            Some(BinKind::Prop) => {}
            Some(found) => {
                return Err(Error::UnexpectedBinKind {
                    expected: BinKind::Prop,
                    found,
                })
            }
            None => return Err(Error::InvalidFileSignature),
        }

        let version = reader.read_u32::<LE>()?;
        if !matches!(version, 1..=3) {
            return Err(Error::InvalidFileVersion(version));
        }

        let dependencies = match version {
            2.. => {
                let dep_count = reader.read_u32::<LE>()?;
                let mut dependencies = Vec::with_capacity(dep_count as _);
                for _ in 0..dep_count {
                    dependencies.push(reader.read_sized_string_u16::<LE>()?);
                }
                dependencies
            }
            _ => Vec::new(),
        };

        let count = reader.read_u32::<LE>()? as usize;
        let mut class_hashes = Vec::with_capacity(count);
        for _ in 0..count {
            class_hashes.push(reader.read_bin_hash::<LE>()?);
        }

        let objects_start = reader.stream_position()?;

        Ok(Self {
            reader,
            version,
            dependencies,
            class_hashes,
            objects_start,
            toc: BinToc::default(),
            legacy: false,
            meta: PhantomData,
        })
    }

    // ── header facts, free after mount ──────────────────────────────────────

    /// The bin file version.
    #[must_use]
    pub fn version(&self) -> u32 {
        self.version
    }

    /// The other property bins this file depends on.
    #[must_use]
    pub fn dependencies(&self) -> &[String] {
        &self.dependencies
    }

    /// Class hash of every object, in file order. `class_hashes().len()` is the object count.
    #[must_use]
    pub fn class_hashes(&self) -> &[BinHash] {
        &self.class_hashes
    }

    // ── sweeping ────────────────────────────────────────────────────────────

    /// A cursor over the object table.
    ///
    /// Every call starts a fresh sweep from the top; cursors hold no state between calls.
    /// Objects not descended into are skipped by their size field, and rows the
    /// [`BinStream::toc`] already holds are served without touching the reader.
    pub fn objects(&mut self) -> Objects<'_, R, M> {
        Objects::new(self)
    }

    /// A `std` iterator of plain [`ObjectEntry`] descriptors, for harvesting and filtering.
    ///
    /// Equivalent to [`BinStream::objects`] without ever descending; restarts the same way.
    pub fn entries(&mut self) -> Entries<'_, R, M> {
        Entries::new(self)
    }

    // ── random access ───────────────────────────────────────────────────────

    /// The table of contents: every object's `(path_hash, class_hash, offset, size)`.
    ///
    /// Built by one harvest sweep on first use, then cached. [`BinStream::objects`] and
    /// [`BinStream::entries`] sweeps also populate it as a side effect, so a fully-swept
    /// handle pays nothing.
    ///
    /// # Errors
    ///
    /// An I/O error from the source if the harvest sweep fails.
    pub fn toc(&mut self) -> Result<&BinToc, Error> {
        let mut index = self.toc.entries().len();
        while index < self.class_hashes.len() {
            self.harvest(index)?;
            index += 1;
        }
        Ok(&self.toc)
    }

    /// Opens the object with the given path hash, building the TOC if needed.
    ///
    /// # Errors
    ///
    /// An I/O error from the source if the TOC harvest fails.
    pub fn object(
        &mut self,
        path_hash: impl Into<BinHash>,
    ) -> Result<Option<ObjectStream<'_, R, M>>, Error> {
        let path_hash = path_hash.into();
        self.toc()?;
        match self.toc.entry(path_hash) {
            Some(&entry) => Ok(Some(ObjectStream::new(self, entry))),
            None => Ok(None),
        }
    }

    // ── teardown ────────────────────────────────────────────────────────────

    /// Returns the underlying source, discarding the internal buffer.
    #[must_use]
    pub fn into_inner(self) -> R {
        self.reader.into_inner()
    }

    // ── internals shared with the cursors ───────────────────────────────────

    /// The TOC row at table position `index`, if a sweep has reached it.
    pub(crate) fn toc_row(&self, index: usize) -> Option<&ObjectEntry> {
        self.toc.entries().get(index)
    }

    /// Reads the 8-byte object header (`size`, `path_hash`) of the first row the TOC does
    /// not hold, and appends it. `index` must be the TOC's current length.
    pub(crate) fn harvest(&mut self, index: usize) -> Result<ObjectEntry, Error> {
        debug_assert_eq!(
            index,
            self.toc.entries().len(),
            "harvest must extend the TOC one row at a time"
        );

        let offset = match self.toc.entries().last() {
            Some(previous) => previous.byte_range().end,
            None => self.objects_start,
        };
        self.seek_to(offset)?;

        let size = self.reader.read_u32::<LE>()?;
        let path_hash = self.reader.read_bin_hash::<LE>()?;
        let entry = ObjectEntry {
            path_hash,
            class_hash: self.class_hashes[index],
            offset,
            size,
        };
        self.toc.push(entry);
        Ok(entry)
    }

    /// Reads an object's `u16` property count, given the absolute offset of that field.
    pub(crate) fn read_property_count(&mut self, offset: u64) -> Result<u16, Error> {
        self.seek_to(offset)?;
        Ok(self.reader.read_u16::<LE>()?)
    }

    /// Positions the reader at the absolute `offset`, staying inside the buffer when the
    /// hop is short.
    fn seek_to(&mut self, offset: u64) -> Result<(), Error> {
        let position = self.reader.stream_position()?;
        self.reader.seek_relative(offset as i64 - position as i64)?;
        Ok(())
    }
}
