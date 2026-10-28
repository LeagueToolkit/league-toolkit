//! Wwise SoundBanks (`.bnk`): the chunk stream, the `BKHD` header and client acceptance.
use std::{
    borrow::Cow,
    fmt,
    io::{self, Read, Write},
};

use byteorder::{ReadBytesExt, WriteBytesExt, LE};

use crate::{
    hierarchy::HIRC,
    media::{DATA, DIDX},
    BankId, Error, Hierarchy, MediaIndex, Result,
};

/// The tag of the header chunk.
const BKHD: [u8; 4] = *b"BKHD";

/// The length of the `BKHD` fields that every version carries.
const BASE_HEADER_LEN: usize = 20;

/// The length of the `BKHD` fields from version 143: the base fields, `bank_type` and `hash`.
const TYPED_HEADER_LEN: usize = 40;

/// The first version whose `BKHD` carries `bank_type` and `hash`.
const TYPED_HEADER_VERSION: BankVersion = BankVersion(143);

/// The lowest version that the client accepts.
const FIRST_ACCEPTED_VERSION: BankVersion = BankVersion(118);

/// The `alignment` of a version 145 bank that the client ships.
const CURRENT_ALIGNMENT: u16 = 16;

/// The first version whose media alignment is the `alignment` field.
const ALIGNED_MEDIA_VERSION: BankVersion = BankVersion(135);

/// The media alignment of a bank below version 135, or with an `alignment` of 0.
const DEFAULT_MEDIA_ALIGNMENT: u32 = 16;

/// The length of a chunk tag and its `u32` size.
const CHUNK_HEADER_LEN: u64 = 8;

/// A bank version: the `dwBankGeneratorVersion` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BankVersion(pub u32);

impl BankVersion {
    /// Version 125.
    pub const V125: Self = Self(125);
    /// Version 132.
    pub const V132: Self = Self(132);
    /// Version 134, the legacy generation of the client.
    pub const V134: Self = Self(134);
    /// Version 145, the current generation of the client.
    pub const V145: Self = Self(145);
    /// The version that the client reads as current.
    pub const CURRENT: Self = Self::V145;

    /// Returns `true` for a legacy version: 118 to 144.
    ///
    /// The client accepts a legacy bank only when its chunks are `DIDX` and `DATA`.
    pub fn is_legacy(self) -> bool {
        (118..=144).contains(&self.0)
    }
}

/// Writes the decimal value.
impl fmt::Display for BankVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// The `dwBankType` field.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BankType(pub u32);

impl BankType {
    /// A user bank: 0.
    pub const USER: Self = Self(0);
    /// An event bank: 30.
    pub const EVENT: Self = Self(30);
    /// A bus bank: 31.
    pub const BUS: Self = Self(31);
}

/// Writes the decimal value.
impl fmt::Display for BankType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// The decoded `BKHD` chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankHeader {
    /// The bank version. The client accepts 118 to 145.
    pub version: BankVersion,
    /// The ID in the header. The client does not read it.
    pub bank_id: BankId,
    /// The `dwLanguageID` field. The client does not read it.
    pub language_id: u32,
    /// The `uAlignment` field. [`BankHeader::media_alignment`] gives the alignment it selects.
    pub alignment: u16,
    /// The `uAltValues` field.
    pub alt_values: u16,
    /// The `dwProjectID` field. The client does not read it.
    pub project_id: u32,
    /// The `dwBankType` field. Zero below version 143.
    pub bank_type: BankType,
    /// The bank hash. Zero below version 143. The client does not read it.
    pub hash: [u8; 16],
    /// The bytes of the body after the decoded fields.
    pub padding: Vec<u8>,
}

impl BankHeader {
    /// Returns a header with zero fields and no padding.
    pub fn new(version: BankVersion, bank_id: BankId) -> Self {
        Self {
            version,
            bank_id,
            language_id: 0,
            alignment: 0,
            alt_values: 0,
            project_id: 0,
            bank_type: BankType::USER,
            hash: [0; 16],
            padding: Vec::new(),
        }
    }

    /// Returns the alignment of media in `DATA`: `alignment` from version 135, else 16.
    ///
    /// An `alignment` of 0 gives 16.
    pub fn media_alignment(&self) -> u32 {
        if self.version >= ALIGNED_MEDIA_VERSION && self.alignment != 0 {
            u32::from(self.alignment)
        } else {
            DEFAULT_MEDIA_ALIGNMENT
        }
    }

    /// Returns `true` when the body carries `bank_type` and `hash`: version 143 and higher.
    fn is_typed(&self) -> bool {
        self.version >= TYPED_HEADER_VERSION
    }

    /// Decodes the body of a `BKHD` chunk.
    fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < BASE_HEADER_LEN {
            return Err(Error::InvalidHeader);
        }
        let mut fields = body;
        let version = BankVersion(fields.read_u32::<LE>()?);
        let mut header = Self::new(version, BankId(fields.read_u32::<LE>()?));
        header.language_id = fields.read_u32::<LE>()?;
        header.alignment = fields.read_u16::<LE>()?;
        header.alt_values = fields.read_u16::<LE>()?;
        header.project_id = fields.read_u32::<LE>()?;
        if header.is_typed() {
            if body.len() < TYPED_HEADER_LEN {
                return Err(Error::InvalidHeader);
            }
            header.bank_type = BankType(fields.read_u32::<LE>()?);
            fields.read_exact(&mut header.hash)?;
        }
        header.padding = fields.to_vec();
        Ok(header)
    }

    /// Returns the length of the decoded fields of the body: the body without the padding.
    fn fields_len(&self) -> usize {
        if self.is_typed() {
            TYPED_HEADER_LEN
        } else {
            BASE_HEADER_LEN
        }
    }

    /// Writes the `BKHD` chunk with `padding` after the decoded fields.
    fn write(&self, writer: &mut impl Write, padding: &[u8]) -> Result<()> {
        write_chunk_header(writer, BKHD, self.fields_len() + padding.len())?;
        writer.write_u32::<LE>(self.version.0)?;
        writer.write_u32::<LE>(self.bank_id.0)?;
        writer.write_u32::<LE>(self.language_id)?;
        writer.write_u16::<LE>(self.alignment)?;
        writer.write_u16::<LE>(self.alt_values)?;
        writer.write_u32::<LE>(self.project_id)?;
        if self.is_typed() {
            writer.write_u32::<LE>(self.bank_type.0)?;
            writer.write_all(&self.hash)?;
        }
        writer.write_all(padding)?;
        Ok(())
    }
}

/// A bank read into memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bank {
    header: BankHeader,
    sections: Vec<Section>,
}

impl Bank {
    /// Returns a bank with the header `header` and no sections.
    pub fn new(header: BankHeader) -> Self {
        Self {
            header,
            sections: Vec::new(),
        }
    }

    /// Reads a bank: the chunks in sequence to the end of the input.
    ///
    /// The reader does not check client acceptance. [`Bank::check`] does.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidHeader`] when the first chunk is not `BKHD`, or its body is shorter
    ///   than the fields of its version.
    /// - [`Error::ChunkOverrun`] when a chunk ends after the end of the input.
    /// - [`Error::MediaOutOfBounds`] when a `DIDX` record ends after the end of `DATA`.
    /// - [`Error::DuplicateSource`] when two `DIDX` records have one source ID.
    /// - [`Error::Framing`] when the objects of `HIRC` do not end at the end of the chunk.
    /// - [`Error::PayloadTooShort`] when an object of `HIRC` is shorter than an object ID.
    /// - [`Error::Io`] when the reader fails.
    pub fn from_reader(reader: &mut impl Read) -> Result<Self> {
        let mut chunks = ChunkReader::new(reader);
        let header = match chunks.next_chunk() {
            Ok(Some(chunk)) if chunk.tag == BKHD => BankHeader::decode(&chunk.body)?,
            Ok(_) => return Err(Error::InvalidHeader),
            Err(Error::ChunkOverrun { tag, .. }) if tag != BKHD => {
                return Err(Error::InvalidHeader)
            }
            Err(error) => return Err(error),
        };

        let mut sections = Vec::new();
        let mut next = chunks.next_chunk()?;
        while let Some(chunk) = next.take() {
            next = chunks.next_chunk()?;
            let section = match next.take_if(|data| chunk.tag == DIDX && data.tag == DATA) {
                Some(data) => {
                    next = chunks.next_chunk()?;
                    Section::Media(MediaIndex::decode(chunk.body, data.body)?)
                }
                None if chunk.tag == HIRC => Section::Hierarchy(Hierarchy::decode(&chunk.body)?),
                None => Section::Raw(RawSection {
                    tag: chunk.tag,
                    data: chunk.body,
                }),
            };
            sections.push(section);
        }
        Ok(Self { header, sections })
    }

    /// Writes `BKHD`, then the sections in order.
    ///
    /// In a bank with a media index, the `DATA` body of the first media index starts at a
    /// multiple of [`BankHeader::media_alignment`] from the start of the file. The writer keeps
    /// the padding of the header when it gives that start. Otherwise it writes the shortest run
    /// of zero bytes that gives it in place of the padding. A bank without a media index writes
    /// the padding as is.
    ///
    /// # Errors
    ///
    /// - [`Error::Io`] when the writer fails, or a chunk is longer than `u32::MAX` bytes.
    pub fn to_writer(&self, writer: &mut impl Write) -> Result<()> {
        let alignment = self.header.media_alignment();
        let padding = match self.data_offset_without_padding(alignment) {
            Some(start)
                if !(start + self.header.padding.len()).is_multiple_of(alignment as usize) =>
            {
                Cow::Owned(vec![0; start.next_multiple_of(alignment as usize) - start])
            }
            Some(_) | None => Cow::Borrowed(self.header.padding.as_slice()),
        };
        self.header.write(writer, &padding)?;
        for section in &self.sections {
            match section {
                Section::Media(media) => media.write(writer, self.header.media_alignment())?,
                Section::Hierarchy(hierarchy) => hierarchy.write(writer)?,
                Section::Raw(raw) => {
                    write_chunk_header(writer, raw.tag, raw.data.len())?;
                    writer.write_all(&raw.data)?;
                }
            }
        }
        Ok(())
    }

    /// Returns the file offset of the `DATA` body of the first media index when `BKHD` has no
    /// padding, or `None` for a bank without a media index.
    ///
    /// `alignment` is the media alignment that the media index is written with.
    fn data_offset_without_padding(&self, alignment: u32) -> Option<usize> {
        let chunk_header = CHUNK_HEADER_LEN as usize;
        let mut offset = chunk_header + self.header.fields_len();
        for section in &self.sections {
            match section {
                Section::Media(media) => {
                    return Some(offset + 2 * chunk_header + media.didx_len(alignment));
                }
                Section::Hierarchy(hierarchy) => offset += chunk_header + hierarchy.encoded_len(),
                Section::Raw(raw) => offset += chunk_header + raw.data.len(),
            }
        }
        None
    }

    /// Returns the header.
    pub fn header(&self) -> &BankHeader {
        &self.header
    }

    /// Returns the header for editing.
    pub fn header_mut(&mut self) -> &mut BankHeader {
        &mut self.header
    }

    /// Returns the sections after `BKHD`, in file order.
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    /// Returns the sections after `BKHD` for editing.
    pub fn sections_mut(&mut self) -> &mut Vec<Section> {
        &mut self.sections
    }

    /// Returns a media bank for the file `name` that holds `media`.
    ///
    /// The header has version 145, the bank ID of `name`, alignment 16 and bank type 0. Every
    /// other field is zero. The bank passes [`Bank::check`].
    ///
    /// ```
    /// use ltk_wwise::{Bank, BankId, MediaIndex};
    ///
    /// let bank = Bank::media_only("sett_base_sfx_audio.bnk", MediaIndex::new());
    /// assert_eq!(bank.header().bank_id, BankId(0xe9b70b40));
    /// assert_eq!(bank.check(), Ok(()));
    /// ```
    pub fn media_only(name: &str, media: MediaIndex) -> Self {
        let mut header = BankHeader::new(BankVersion::CURRENT, BankId::from_name(name));
        header.alignment = CURRENT_ALIGNMENT;
        Self {
            header,
            sections: vec![Section::Media(media)],
        }
    }

    /// Returns the first condition under which the client rejects the bank.
    ///
    /// The conditions are tested in the order of the client: the version, the bank type, then
    /// each section in order.
    pub fn check(&self) -> std::result::Result<(), Rejection> {
        let version = self.header.version;
        if !(FIRST_ACCEPTED_VERSION..=BankVersion::CURRENT).contains(&version) {
            return Err(Rejection::Version(version));
        }
        let bank_type = self.header.bank_type;
        if !matches!(bank_type, BankType::USER | BankType::EVENT | BankType::BUS) {
            return Err(Rejection::BankType(bank_type));
        }
        if version.is_legacy() {
            for section in &self.sections {
                let tag = match section {
                    Section::Media(_) => continue,
                    Section::Hierarchy(_) => HIRC,
                    Section::Raw(raw) if raw.tag == DIDX || raw.tag == DATA => continue,
                    Section::Raw(raw) => raw.tag,
                };
                return Err(Rejection::LegacyChunk { version, tag });
            }
        }
        Ok(())
    }

    /// Returns the first media index.
    pub fn media(&self) -> Option<&MediaIndex> {
        self.sections.iter().find_map(|section| match section {
            Section::Media(media) => Some(media),
            Section::Hierarchy(_) | Section::Raw(_) => None,
        })
    }

    /// Returns the first media index for editing.
    pub fn media_mut(&mut self) -> Option<&mut MediaIndex> {
        self.sections.iter_mut().find_map(|section| match section {
            Section::Media(media) => Some(media),
            Section::Hierarchy(_) | Section::Raw(_) => None,
        })
    }

    /// Returns the first hierarchy.
    pub fn hierarchy(&self) -> Option<&Hierarchy> {
        self.sections.iter().find_map(|section| match section {
            Section::Hierarchy(hierarchy) => Some(hierarchy),
            Section::Media(_) | Section::Raw(_) => None,
        })
    }

    /// Returns the first hierarchy for editing.
    pub fn hierarchy_mut(&mut self) -> Option<&mut Hierarchy> {
        self.sections.iter_mut().find_map(|section| match section {
            Section::Hierarchy(hierarchy) => Some(hierarchy),
            Section::Media(_) | Section::Raw(_) => None,
        })
    }
}

/// One section of a bank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Section {
    /// A `DIDX` chunk and the `DATA` chunk after it.
    Media(MediaIndex),
    /// A `HIRC` chunk.
    Hierarchy(Hierarchy),
    /// Any other chunk, kept as bytes.
    Raw(RawSection),
}

/// A chunk that the crate keeps as bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSection {
    /// The chunk tag.
    pub tag: [u8; 4],
    /// The chunk body.
    pub data: Vec<u8>,
}

/// A condition under which the client rejects a bank.
///
/// The retail client rejects such a bank with `AK_WrongBankVersion` and logs nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
#[non_exhaustive]
pub enum Rejection {
    /// The version is below 118 or above 145.
    #[error("bank version {0} is not 118 to 145")]
    Version(BankVersion),
    /// A legacy version and a chunk other than `DIDX` or `DATA`.
    #[error("legacy bank version {version} has a {} chunk", .tag.escape_ascii())]
    LegacyChunk {
        /// The version of the bank.
        version: BankVersion,
        /// The tag of the first such chunk.
        tag: [u8; 4],
    },
    /// A bank type other than 0, 30 or 31.
    #[error("bank type {0} is not 0, 30 or 31")]
    BankType(BankType),
}

/// One chunk as read.
struct Chunk {
    tag: [u8; 4],
    body: Vec<u8>,
}

/// Reads the chunks of a bank in sequence and tracks the offset of the next chunk.
struct ChunkReader<'a, R> {
    reader: &'a mut R,
    offset: u64,
}

impl<'a, R: Read> ChunkReader<'a, R> {
    fn new(reader: &'a mut R) -> Self {
        Self { reader, offset: 0 }
    }

    /// Reads the next chunk. Returns `None` at the end of the input.
    fn next_chunk(&mut self) -> Result<Option<Chunk>> {
        let mut head = [0; CHUNK_HEADER_LEN as usize];
        let head_len = read_full(self.reader, &mut head)?;
        let mut tag = [0; 4];
        tag.copy_from_slice(&head[..4]);
        if head_len == 0 {
            return Ok(None);
        }
        let overrun = Error::ChunkOverrun {
            tag,
            offset: self.offset,
        };
        if head_len < head.len() {
            return Err(overrun);
        }
        let size = u32::from_le_bytes([head[4], head[5], head[6], head[7]]);

        let mut body = Vec::new();
        self.reader.take(u64::from(size)).read_to_end(&mut body)?;
        if body.len() as u64 != u64::from(size) {
            return Err(overrun);
        }
        self.offset += CHUNK_HEADER_LEN + u64::from(size);
        Ok(Some(Chunk { tag, body }))
    }
}

/// Reads into `buf` until it is full or the input ends. Returns the number of bytes read.
fn read_full(reader: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(filled)
}

/// Writes a chunk tag and the `u32` size of a body of `len` bytes.
pub(crate) fn write_chunk_header(writer: &mut impl Write, tag: [u8; 4], len: usize) -> Result<()> {
    let size = u32::try_from(len).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "chunk {} of {len} bytes is longer than u32::MAX",
                tag.escape_ascii()
            ),
        )
    })?;
    writer.write_all(&tag)?;
    writer.write_u32::<LE>(size)?;
    Ok(())
}
