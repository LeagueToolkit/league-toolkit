use std::{
    fmt,
    io::{self, Seek as _},
    marker::PhantomData,
    sync::Arc,
};

use byteorder::{ReadBytesExt as _, LE};
use indexmap::IndexMap;
use ltk_hash::{BinHash, ReadBytesExt as _};
use ltk_io_ext::ReaderExt as _;

use crate::{
    property::NoMeta,
    stream::{
        layout::{Cursor, Numbering},
        owned, BinToc, Entries, NoCache, ObjectCache, ObjectEntry, ObjectStream, Objects,
    },
    Bin, BinKind, BinObject, Error,
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
pub struct BinStream<R: io::Read + io::Seek, M = NoMeta> {
    reader: io::BufReader<R>,
    version: u32,
    dependencies: Vec<String>,
    class_hashes: Vec<BinHash>,
    /// Absolute offset of the first object's `u32` size field.
    objects_start: u64,
    /// Grows as sweeps discover rows; complete once it holds every object.
    toc: BinToc,
    /// One object's declared byte range, reused across descents.
    buffer: Vec<u8>,
    /// The kind-numbering latch. Mounting starts in the current numbering, and the first
    /// object whose kind bytes only make sense in the old one flips it for good.
    numbering: Numbering,
    cache: Box<dyn ObjectCache<M> + Send>,
    meta: PhantomData<fn() -> M>,
}

impl<R: io::Read + io::Seek + fmt::Debug, M> fmt::Debug for BinStream<R, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BinStream")
            .field("reader", &self.reader)
            .field("version", &self.version)
            .field("dependencies", &self.dependencies)
            .field("objects", &self.class_hashes.len())
            .field("harvested", &self.toc.entries().len())
            .field("numbering", &self.numbering)
            .finish_non_exhaustive()
    }
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
            buffer: Vec::new(),
            numbering: Numbering::Current,
            cache: Box::new(NoCache),
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

    /// Which property-kind numbering the handle is reading under.
    ///
    /// Mounting starts at [`Numbering::Current`]. The first object whose kind bytes only
    /// decode as [`Numbering::Legacy`] latches this for the rest of the handle's life, and
    /// views created before the flip keep the numbering they were built under.
    ///
    /// As with the eager reader's retry, a genuinely desynced file can be reinterpreted as
    /// legacy rather than reported as broken; this is how to tell that happened.
    #[must_use]
    pub fn numbering(&self) -> Numbering {
        self.numbering
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

    // ── cached lookup ───────────────────────────────────────────────────────

    /// Resolves an object through the installed [`ObjectCache`].
    ///
    /// A hit is an [`Arc`] clone with no I/O; a miss parses and inserts. Under the default
    /// [`NoCache`] every call is a miss, so every call parses.
    ///
    /// Only this path consults the cache. The cursors and [`BinStream::object`] never do, so a
    /// sweep cannot evict what a consumer is holding hot — and an editor that wants exclusive
    /// ownership of an object takes [`ObjectStream::read`] rather than a shared `Arc`.
    ///
    /// # Errors
    ///
    /// The same as [`ObjectStream::read`], plus an I/O error from the TOC harvest.
    pub fn cached_object(
        &mut self,
        path_hash: impl Into<BinHash>,
    ) -> Result<Option<Arc<BinObject<M>>>, Error> {
        let path_hash = path_hash.into();
        if let Some(hit) = self.cache.get(path_hash) {
            return Ok(Some(hit));
        }

        let object = match self.object(path_hash)? {
            Some(mut object) => object.read()?,
            None => return Ok(None),
        };

        let object = Arc::new(object);
        self.cache.put(path_hash, Arc::clone(&object));
        Ok(Some(object))
    }

    /// Installs a cache provider, dropping whatever the previous one held.
    ///
    /// The default is [`NoCache`].
    pub fn set_cache(&mut self, cache: Box<dyn ObjectCache<M> + Send>) {
        self.cache = cache;
    }

    // ── upgrade / teardown ──────────────────────────────────────────────────

    /// Parses the whole file into an eager [`Bin`], consuming the handle.
    ///
    /// Always processes the object table from the top, whatever the cursors did before it.
    /// This is what [`Bin::from_reader`] is: the stream is the crate's only parser, so the two
    /// can never drift.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidSize`] if an object's declared size disagrees with what its property
    /// counts consumed, whatever the value model raises for a container it refuses
    /// ([`Error::InvalidNesting`], [`Error::InvalidKeyType`],
    /// [`Error::MismatchedContainerTypes`]), or an I/O error from the source.
    pub fn into_bin(mut self) -> Result<Bin<M>, Error> {
        let objects = self.drain_objects()?;
        Ok(Bin {
            version: self.version,
            objects,
            dependencies: self.dependencies,
        })
    }

    /// Returns the underlying source, discarding the internal buffer.
    #[must_use]
    pub fn into_inner(self) -> R {
        self.reader.into_inner()
    }

    /// Reads every object in the table, from the top.
    ///
    /// A latch onto the legacy numbering part-way through invalidates everything read so far,
    /// so the drain starts over — which reproduces the eager reader's whole-table retry. The
    /// latch only ever flips once, so the restart happens at most once.
    fn drain_objects(&mut self) -> Result<IndexMap<BinHash, BinObject<M>>, Error> {
        let mut objects = IndexMap::with_capacity(self.class_hashes.len());

        let mut index = 0;
        while index < self.class_hashes.len() {
            let entry = match self.toc_row(index) {
                Some(&entry) => entry,
                None => self.harvest(index)?,
            };

            let before = self.numbering;
            let object = self.read_object(entry)?;
            if self.numbering != before {
                objects.clear();
                index = 0;
                continue;
            }

            objects.insert(object.path_hash, object);
            index += 1;
        }

        Ok(objects)
    }

    // ── internals shared with the cursors ───────────────────────────────────

    /// The TOC row at table position `index`, if a sweep has reached it.
    pub(crate) fn toc_row(&self, index: usize) -> Option<&ObjectEntry> {
        self.toc.entries().get(index)
    }

    /// Buffers `entry` and walks it, returning a cursor over the bytes it proved.
    ///
    /// The walk is what proves the object before anything reads inside it: it settles the
    /// numbering latch and raises [`Error::InvalidSize`] for a declared size the property
    /// counts disagree with. A view handed out from here therefore never has to check a size
    /// of its own.
    pub(crate) fn view_object(&mut self, entry: ObjectEntry) -> Result<Cursor<'_>, Error> {
        self.load_object(entry)?;
        self.settle(entry.path_hash, |mut cur| cur.walk_object())?;
        Ok(self.buffered())
    }

    /// Buffers `entry` and decodes it.
    ///
    /// No separate walk: the decode is count-driven over the same sized regions, so it raises
    /// everything the walk would and the eager path crosses each object's bytes once.
    pub(crate) fn read_object(&mut self, entry: ObjectEntry) -> Result<BinObject<M>, Error> {
        self.load_object(entry)?;
        self.settle(entry.path_hash, |mut cur| {
            owned::read_object(&mut cur, entry.class_hash)
        })
    }

    /// Reads `entry`'s declared byte range into the reused buffer.
    fn load_object(&mut self, entry: ObjectEntry) -> Result<(), Error> {
        let range = entry.byte_range();
        self.seek_to(range.start)?;

        // The declared size is the file's word, not a fact, so the buffer grows to it a chunk
        // at a time rather than reserving it up front: a lying size costs a short read, not a
        // gigabyte of zeroed memory.
        let want = (range.end - range.start) as usize;
        self.buffer.clear();
        owned::fill_to(&mut self.reader, &mut self.buffer, want)?;
        match self.buffer.len() < want {
            true => Err(Error::IOError(io::Error::from(
                io::ErrorKind::UnexpectedEof,
            ))),
            false => Ok(()),
        }
    }

    /// A cursor over the buffered object, under the handle's numbering.
    fn buffered(&self) -> Cursor<'_> {
        Cursor::new(&self.buffer, self.numbering)
    }

    /// Runs `attempt` over the buffered object, latching onto the legacy numbering if that is
    /// the only one the bytes make sense under.
    ///
    /// Only a kind byte can mean "this file uses the old numbering", and only if the handle
    /// has not already settled the question. The retry costs no I/O - the bytes are already in
    /// memory - and the latch only ever flips one way, so this asks at most once per handle.
    fn settle<T>(
        &mut self,
        path_hash: BinHash,
        attempt: impl Fn(Cursor<'_>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let error = match attempt(self.buffered()) {
            Ok(value) => return Ok(value),
            Err(error @ Error::InvalidPropertyTypePrimitive(_)) if !self.numbering.is_legacy() => {
                error
            }
            Err(error) => return Err(error),
        };

        match attempt(Cursor::new(&self.buffer, Numbering::Legacy)) {
            Ok(value) => {
                log::warn!(
                    "object {path_hash:08x}: invalid property kind, reading the rest of this \
                     bin with the legacy numbering"
                );
                self.numbering = Numbering::Legacy;
                Ok(value)
            }
            // The legacy numbering does not explain it either, so the original complaint is
            // the honest one.
            Err(_) => Err(error),
        }
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
