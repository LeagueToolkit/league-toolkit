//! Reading a bin file without knowing up front which kind it is.

use std::{
    fmt,
    io::{self, SeekFrom},
};

use byteorder::{ReadBytesExt as _, LE};
use indexmap::IndexMap;
use ltk_hash::BinHash;

use crate::{property::NoMeta, Bin, BinObject, BinOverride, Error};

/// The two kinds of bin file, told apart by their magic.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinKind {
    /// A `PROP` bin: the objects themselves. See [`Bin`].
    Prop,
    /// A `PTCH` bin: a patch over a `PROP` bin. See [`BinOverride`].
    Override,
}

impl BinKind {
    /// The four magic bytes at the start of a file of this kind.
    ///
    /// # Examples
    ///
    /// ```
    /// use ltk_meta::BinKind;
    ///
    /// assert_eq!(&BinKind::Prop.magic(), b"PROP");
    /// assert_eq!(&BinKind::Override.magic(), b"PTCH");
    /// ```
    #[must_use]
    pub const fn magic(self) -> [u8; 4] {
        match self {
            Self::Prop => *b"PROP",
            Self::Override => *b"PTCH",
        }
    }

    /// The kind a file with these magic bytes has, if any.
    #[must_use]
    pub fn from_magic(magic: [u8; 4]) -> Option<Self> {
        match &magic {
            b"PROP" => Some(Self::Prop),
            b"PTCH" => Some(Self::Override),
            _ => None,
        }
    }

    /// The kind of bin this data holds, if any. Only the magic is read.
    ///
    /// # Examples
    ///
    /// ```
    /// use ltk_meta::BinKind;
    ///
    /// assert_eq!(BinKind::identify_from_bytes(b"PTCH\x01\0\0\0"), Some(BinKind::Override));
    /// assert_eq!(BinKind::identify_from_bytes(b"OEGM"), None);
    /// ```
    #[must_use]
    pub fn identify_from_bytes(data: &[u8]) -> Option<Self> {
        Self::from_magic(data.get(..4)?.try_into().ok()?)
    }

    /// The kind of bin this reader holds, leaving the reader where it was.
    ///
    /// Answers "which one do I call" for a `.bin` that could be either. Both
    /// [`Bin::from_reader`] and [`BinOverride::from_reader`] still expect the magic, which is
    /// where this leaves the reader.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidFileSignature`] if the magic belongs to neither kind.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use ltk_meta::{Bin, BinKind, BinOverride};
    ///
    /// let mut file = File::open("unknown.bin")?;
    /// match BinKind::identify_from_reader(&mut file)? {
    ///     BinKind::Prop => {
    ///         let bin = Bin::from_reader(&mut file)?;
    ///     }
    ///     BinKind::Override => {
    ///         let patch_bin = BinOverride::from_reader(&mut file)?;
    ///     }
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn identify_from_reader<R: io::Read + io::Seek + ?Sized>(
        reader: &mut R,
    ) -> Result<Self, Error> {
        let start = reader.stream_position()?;
        let magic = reader.read_u32::<LE>()?;
        reader.seek(SeekFrom::Start(start))?;

        Self::from_magic_u32(magic).ok_or(Error::InvalidFileSignature)
    }

    pub(crate) fn from_magic_u32(magic: u32) -> Option<Self> {
        Self::from_magic(magic.to_le_bytes())
    }
}

impl fmt::Display for BinKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Prop => "PROP",
            Self::Override => "PTCH",
        })
    }
}

/// A bin file of either kind.
///
/// Use this when a file's kind is not known in advance, as when walking a wad archive or opening
/// a `.bin` by extension. To pick a reader yourself instead of holding the enum, ask
/// [`BinKind::identify_from_reader`] first.
///
/// # Examples
///
/// ```no_run
/// use std::fs::File;
/// use ltk_meta::BinFile;
///
/// let mut reader = File::open("unknown.bin")?;
/// let file = BinFile::from_reader(&mut reader)?;
///
/// // Whichever kind it is, these are its objects.
/// println!("{} objects", file.objects().len());
///
/// match file {
///     BinFile::Prop(bin) => println!("{} dependencies", bin.dependencies.len()),
///     BinFile::Override(patch_bin) => println!("{} patches", patch_bin.patches.len()),
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(bound = "for <'dee> M: serde::Serialize + serde::Deserialize<'dee>")
)]
#[derive(Debug, Clone, PartialEq)]
pub enum BinFile<M = NoMeta> {
    /// A `PROP` bin.
    Prop(Bin<M>),
    /// A `PTCH` bin.
    Override(BinOverride<M>),
}

impl BinFile {
    /// Reads a bin file of either kind, choosing by the file's magic.
    ///
    /// # Arguments
    ///
    /// * `reader` - A reader that implements [`io::Read`] and [`io::Seek`].
    pub fn from_reader<R: io::Read + io::Seek + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        match BinKind::identify_from_reader(reader)? {
            BinKind::Prop => Ok(Self::Prop(Bin::from_reader(reader)?)),
            BinKind::Override => Ok(Self::Override(BinOverride::from_reader(reader)?)),
        }
    }
}

impl<M> BinFile<M> {
    /// The kind of file this is.
    #[must_use]
    #[inline]
    pub fn kind(&self) -> BinKind {
        match self {
            Self::Prop(_) => BinKind::Prop,
            Self::Override(_) => BinKind::Override,
        }
    }

    /// Whether this is a `PROP` bin.
    #[must_use]
    #[inline]
    pub fn is_prop(&self) -> bool {
        matches!(self, Self::Prop(_))
    }

    /// Whether this is a `PTCH` bin.
    #[must_use]
    #[inline]
    pub fn is_override(&self) -> bool {
        matches!(self, Self::Override(_))
    }

    /// The `PROP` bin, if this is one.
    #[must_use]
    #[inline]
    pub fn as_prop(&self) -> Option<&Bin<M>> {
        match self {
            Self::Prop(bin) => Some(bin),
            Self::Override(_) => None,
        }
    }

    /// See [`BinFile::as_prop`].
    #[must_use]
    #[inline]
    pub fn as_prop_mut(&mut self) -> Option<&mut Bin<M>> {
        match self {
            Self::Prop(bin) => Some(bin),
            Self::Override(_) => None,
        }
    }

    /// See [`BinFile::as_prop`].
    #[must_use]
    #[inline]
    pub fn into_prop(self) -> Option<Bin<M>> {
        match self {
            Self::Prop(bin) => Some(bin),
            Self::Override(_) => None,
        }
    }

    /// The `PTCH` bin, if this is one.
    #[must_use]
    #[inline]
    pub fn as_override(&self) -> Option<&BinOverride<M>> {
        match self {
            Self::Override(patch_bin) => Some(patch_bin),
            Self::Prop(_) => None,
        }
    }

    /// See [`BinFile::as_override`].
    #[must_use]
    #[inline]
    pub fn as_override_mut(&mut self) -> Option<&mut BinOverride<M>> {
        match self {
            Self::Override(patch_bin) => Some(patch_bin),
            Self::Prop(_) => None,
        }
    }

    /// See [`BinFile::as_override`].
    #[must_use]
    #[inline]
    pub fn into_override(self) -> Option<BinOverride<M>> {
        match self {
            Self::Override(patch_bin) => Some(patch_bin),
            Self::Prop(_) => None,
        }
    }

    /// The objects the file holds, whichever kind it is.
    ///
    /// For a `PTCH` these are the objects it adds, not the ones it patches.
    #[must_use]
    #[inline]
    pub fn objects(&self) -> &IndexMap<BinHash, BinObject<M>> {
        match self {
            Self::Prop(bin) => &bin.objects,
            Self::Override(patch_bin) => &patch_bin.objects,
        }
    }

    /// See [`BinFile::objects`].
    #[must_use]
    #[inline]
    pub fn objects_mut(&mut self) -> &mut IndexMap<BinHash, BinObject<M>> {
        match self {
            Self::Prop(bin) => &mut bin.objects,
            Self::Override(patch_bin) => &mut patch_bin.objects,
        }
    }
}

impl<M: Clone> BinFile<M> {
    /// Writes this file back out in its own format.
    ///
    /// # Arguments
    ///
    /// * `writer` - A writer that implements [`io::Write`] and [`io::Seek`].
    pub fn to_writer<W: io::Write + io::Seek + ?Sized>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Prop(bin) => bin.to_writer(writer),
            Self::Override(patch_bin) => patch_bin.to_writer(writer),
        }
    }
}

impl<M> From<Bin<M>> for BinFile<M> {
    fn from(value: Bin<M>) -> Self {
        Self::Prop(value)
    }
}

impl<M> From<BinOverride<M>> for BinFile<M> {
    fn from(value: BinOverride<M>) -> Self {
        Self::Override(value)
    }
}
