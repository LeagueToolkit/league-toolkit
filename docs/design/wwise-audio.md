# Wwise audio in `ltk_wwise`

The API spec for the audio files of the client in `ltk_wwise`: the package format, the bank
format, the hierarchy layouts, the event query, the version upgrade and the rules of the reader
and the writer.

This document states the behavior of `ltk_wwise`. If the code and a section of this document
disagree, the section is wrong and must be edited. ADR-0031 records the crate decision. The rules
table in [section 16](#s16) cites it.

## <a id="s1"></a>1. Summary

The client plays all sound through Audiokinetic Wwise, linked statically. The bank reader of the
16.17 client is version 145. A skin, a map, a feature or a spell names its audio with a list of
`BankUnit` objects in a bin. A `BankUnit` holds a list of paths. A path names one of three files:

| File | Contents | Client behavior |
| --- | --- | --- |
| media bank, `*_audio.bnk` | `BKHD`, `DIDX`, `DATA` | Loaded into memory. Accepted at bank version 118 to 145. |
| event bank, `*_events.bnk` | `BKHD`, `HIRC` | Loaded into memory. Accepted at bank version 145 only. |
| package, `*.wpk` | Riot container of WEM files | Read as a WAD substream. Each WEM is found by its source ID. |

`ltk_wwise` reads each file into an owned value and writes the value back. A file that the caller
does not edit writes back byte for byte. The crate edits the media of a package and of a media
bank by source ID ([section 5](#s5) and [section 7](#s7)). It exposes every hierarchy object as a
kind, an ID and a payload ([section 8](#s8)). It decodes the objects that a mod edits
([section 9](#s9)). It resolves an event to the WEM sources that it plays ([section 10](#s10)). It
rewrites a version 132 or 134 bank as a version 145 bank ([section 11](#s11)).

`ltk_wwise` does not decode audio. A WEM is opaque bytes, except for its format chunk
([section 13](#s13)).

## <a id="s2"></a>2. Vocabulary

- **client** - the game executable.
- **bank** - a Wwise SoundBank file, extension `.bnk`. The type is `Bank`.
- **media bank** - a bank with `DIDX` and `DATA` and no `HIRC`. Riot names it `*_audio.bnk`.
- **event bank** - a bank with `HIRC`. Riot names it `*_events.bnk`.
- **package** - a Riot WPK file, extension `.wpk`, magic `r3d2`. The type is `Package`. The crate
  does not use the word "archive" for a package. An archive is a WAD.
- **chunk** - one tagged record of a bank: a 4-byte tag, a `u32` size and a body.
- **section** - a chunk, or the `DIDX` and `DATA` pair, as the crate holds it. The type is
  `Section`.
- **bank version** - the `dwBankGeneratorVersion` field of `BKHD`. The type is `BankVersion`.
- **legacy version** - a bank version from 118 to 144.
- **media** - the bytes of one WEM file.
- **source ID** - the ID of one media. The type is `SourceId`. A media bank indexes media by
  source ID. A package names each media `<source ID>.wem`.
- **media index** - the `DIDX` and `DATA` pair of a bank. The type is `MediaIndex`.
- **hierarchy** - the `HIRC` chunk: the list of objects of an event bank. The type is `Hierarchy`.
- **object** - one entry of the hierarchy: a kind, an ID and a payload. The type is `Object`.
  Audiokinetic calls it an item. The crate uses "object".
- **object ID** - the first `u32` of a payload. The type is `ObjectId`. The object ID of an event
  is the FNV-1 hash of the event name.
- **kind** - the type byte of an object. The type is `ObjectKind`. The crate does not use the word
  "type" for it. "Type" means a Rust type.
- **node base** - the block of parameters that every node object carries: Sound, the containers,
  ActorMixer and the music objects ([section 9.3](#s9.3)).
- **layout generation** - a set of bank versions that share one set of payload layouts. Version
  132 and version 134 are the legacy generation. Version 145 is the current generation.
- **bank ID** - the ID under which the client registers a bank. The type is `BankId`. The client
  computes it from the file name ([section 4.2](#s4.2)).
- **package key** - the key under which the client registers a package entry. The type is
  `PackageKey` ([section 5.2](#s5.2)).
- **upgrade** - the rewrite of a legacy-generation bank as a version 145 bank ([section 11](#s11)).

## <a id="s3"></a>3. Evidence

The rules of this document come from three sources.

1. **The client.** Retail 16.17.8057408 contains the version 145 bank reader. The 2024 all-logs
   debug build contains the version 134 bank reader. Both builds link Wwise as a release static
   library. Only the exported `AK::SoundEngine` symbols survive. [Appendix A](#appendix-a) lists
   the functions.
2. **The `league_structs` reversing notes** `Wwise_BankHeader_VersionGate`, `Wwise_BankIdHash`,
   `Wwise_HircFormat_134_to_145`, `LocaleResolution_LocalizedAssets` and `WadLoader_MountAndRead`,
   and the layout tool `wwise_hirc_layout`. The tool checks each layout by exact byte accounting:
   a layout is correct when it consumes every byte of every object.
3. **Shipped files.** [Appendix B](#appendix-b) holds the measurements, each with its client
   build.

No upgraded bank has been loaded in a match. An upgraded bank is what the version 145 reader
parses to the last byte. In-game playback of an upgraded bank is unverified.

## <a id="s4"></a>4. Crate surface

`ltk_wwise` has these modules. The crate root re-exports the types in bold.

| Module | Contents | Spec |
| --- | --- | --- |
| root | **`ObjectId`**, **`SourceId`**, **`BankId`**, **`Error`**, **`Result`** | [section 4.1](#s4.1), [section 14](#s14) |
| `package` | **`Package`**, `PackageEntry`, `PackageKey`, **`MountedPackage`**, `MountedEntry` | [section 5](#s5) |
| `bank` | **`Bank`**, `BankHeader`, `BankVersion`, `BankType`, `Section`, `RawSection`, `Rejection` | [section 6](#s6) |
| `media` | **`MediaIndex`** | [section 7](#s7) |
| `hierarchy` | **`Hierarchy`**, `Object`, `ObjectKind`, the typed objects, `event_sources` | [section 8](#s8) to [section 10](#s10) |
| `upgrade` | `UpgradeRefusal` and the upgrade transforms | [section 11](#s11) |
| `path` | `AudioPath`, `AudioFileKind` | [section 12](#s12) |
| `wem` | `WemInfo`, `WemCodec` | [section 13](#s13) |

The crate depends on `ltk_hash`, `byteorder` and `thiserror`. `league-toolkit` re-exports it as
`wwise` with the `wwise` feature.

### <a id="s4.1"></a>4.1 IDs

```rust
/// A Wwise object ID: the first `u32` of a hierarchy object.
///
/// The ID of an event, a switch, a state or a game parameter is the FNV-1 hash of its name.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(pub u32);

impl ObjectId {
    /// Returns the ID of the name `name`: [`ltk_hash::fnv1::hash_lower`].
    pub fn from_name(name: &str) -> Self;
}

/// The ID of one media, in a media index or in a package entry name.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceId(pub u32);

/// The ID under which the client registers a bank.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BankId(pub u32);

impl BankId {
    /// Returns the ID of the bank file `name`: [`ltk_hash::fnv1::bank_id`].
    pub fn from_name(name: &str) -> Self;
}
```

The three types implement `From<u32>` and `Display`. `Display` writes the decimal value. The
package entry names and the Wwise authoring tool show IDs in decimal.

### <a id="s4.2"></a>4.2 The FNV-1 hash

`ltk_hash::fnv1` contains the Wwise hash.

```rust
/// Returns the FNV-1 32-bit hash of `input` with `A` to `Z` lowercased.
pub const fn hash_lower(input: &str) -> u32;

/// Returns the bank ID of the bank file name `name`.
///
/// Truncates `name` to 259 bytes, cuts it at its last `.`, and hashes the rest with
/// [`hash_lower`].
pub fn bank_id(name: &str) -> u32;
```

The hash starts at `0x811C9DC5`. For each byte it multiplies by `0x01000193`, then XORs the byte.
FNV-1a does the two steps in the opposite order. The lowercasing adds 32 to the bytes `A` to `Z`
and keeps every other byte. The client function `AK::SoundEngine::GetIDFromString` is this hash.

`bank_id` is the client function `GetBankIDFromString`. It copies the name into a buffer of 260
bytes, which holds 259 bytes and a NUL. It writes a NUL at the last `.`. A name without a `.` is
hashed whole. `bank_id("sett_base_sfx_audio.bnk")` and `bank_id("sett_base_sfx_audio")` are both
`0xE9B70B40`. `bank_id` does not remove a directory. [Section 12](#s12) takes the file name of a
path.

## <a id="s5"></a>5. Package

### <a id="s5.1"></a>5.1 Wire format (normative)

All integers are little-endian.

```text
Package
  [u8; 4]   magic           "r3d2"
  u32       version         1
  u32       entry_count
  u32       entry_offset[entry_count]     absolute offset of each entry record

Entry record (at entry_offset[i], 8-byte aligned)
  u32       data_offset     absolute offset of the media, 8-byte aligned
  u32       data_size
  u32       name_length     in UTF-16 code units
  u16       name[name_length]             UTF-16LE, no terminator
```

The first entry record starts at the first 8-byte boundary after the offset table. Each next
record starts at the first 8-byte boundary after the previous record. The data region starts at
the first 8-byte boundary after the last record. Each media starts at the first 8-byte boundary
after the previous media. Every padding byte is zero. The file ends at the end of the last media.

Each media is a RIFF file with the form type `WAVE`. Shipped names are `<source ID>.wem`, the
source ID in decimal.

### <a id="s5.2"></a>5.2 The package key

```rust
/// The key under which the client registers a package entry.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageKey(pub u64);

impl PackageKey {
    /// Returns the key of the entry name `name`.
    pub fn from_name(name: &str) -> Self;
}
```

The key of a name is the leading run of ASCII decimal digits of the name, read as a `u64`. When
that value is 0, or the run is empty, or the value exceeds `u64::MAX`, the key is the ELF hash
(`ltk_hash::elf`) of the name with `A` to `Z` lowercased. `1004362275.wem` has the key
1004362275. The client function `Wpk_ParseDirectory` computes the key.

### <a id="s5.3"></a>5.3 `Package`

```rust
/// A package read into memory.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Package { /* entries in order */ }

impl Package {
    pub fn new() -> Self;
    pub fn from_reader(reader: &mut impl Read) -> Result<Self>;
    pub fn to_writer(&self, writer: &mut impl Write) -> Result<()>;

    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    /// Returns the entries in order.
    pub fn entries(&self) -> &[PackageEntry];
    pub fn get(&self, key: PackageKey) -> Option<&PackageEntry>;
    /// Returns the media of the entry with the key of `source`.
    pub fn wem(&self, source: SourceId) -> Option<&[u8]>;

    /// Inserts `entry`. Replaces the entry with the same key in place and returns it.
    pub fn insert(&mut self, entry: PackageEntry) -> Option<PackageEntry>;
    /// Inserts the media `data` as `<source>.wem`. Returns the media it replaces.
    pub fn insert_wem(&mut self, source: SourceId, data: Vec<u8>) -> Option<Vec<u8>>;
    pub fn remove(&mut self, key: PackageKey) -> Option<PackageEntry>;
}

/// One entry of a package: a name and a media.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageEntry { /* name, data */ }

impl PackageEntry {
    pub fn new(name: impl Into<String>, data: Vec<u8>) -> Self;
    /// Returns the entry `<source>.wem`.
    pub fn wem(source: SourceId, data: Vec<u8>) -> Self;
    pub fn name(&self) -> &str;
    pub fn key(&self) -> PackageKey;
    pub fn data(&self) -> &[u8];
    pub fn into_data(self) -> Vec<u8>;
}
```

A package holds at most one entry per key. `insert` of a new key appends the entry. The reader
rejects a file with two entries of one key ([section 14](#s14)). The reader rejects a magic other
than `r3d2` and a version other than 1. The reader rejects an entry record or a media that ends
after the end of the file.

### <a id="s5.4"></a>5.4 `MountedPackage`

```rust
/// A package whose entry records are read and whose media stay in the source.
#[derive(Debug)]
pub struct MountedPackage<R> { /* source, entries */ }

impl<R: Read + Seek> MountedPackage<R> {
    /// Reads the header and the entry records of the package in `source`.
    pub fn mount(source: R) -> Result<Self>;
    pub fn entries(&self) -> &[MountedEntry];
    pub fn get(&self, key: PackageKey) -> Option<&MountedEntry>;
    /// Reads the media of `entry` from the source.
    pub fn read(&mut self, entry: &MountedEntry) -> Result<Vec<u8>>;
    /// Reads every media and returns the package.
    pub fn to_package(&mut self) -> Result<Package>;
    pub fn into_inner(self) -> R;
}

/// The entry record of a mounted package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountedEntry { /* name, offset, size */ }

impl MountedEntry {
    pub fn name(&self) -> &str;
    pub fn key(&self) -> PackageKey;
    pub fn offset(&self) -> u32;
    pub fn size(&self) -> u32;
}
```

`mount` applies the checks of [section 5.3](#s5.3) to the header and the records. It checks the
media bounds against the length of the source. `read` seeks to the media and reads `size` bytes.
The `ltk_wad` type `Wad` is the model: `Wad::mount` reads the table of contents and leaves the
chunks in the source.

### <a id="s5.5"></a>5.5 Writing

`Package::to_writer` writes the layout of [section 5.1](#s5.1) with the entries in order. A package
read from a shipped file writes back byte for byte ([appendix B](#appendix-b)).

## <a id="s6"></a>6. Bank

### <a id="s6.1"></a>6.1 Chunk stream (normative)

```text
Bank
  Chunk[]   until the end of the file

Chunk
  [u8; 4]   tag             "BKHD", "DIDX", "DATA", "HIRC", "STID", "INIT", "STMG", "ENVS", "PLAT", ...
  u32       size
  [u8; size] body
```

The first chunk is `BKHD`. Each next chunk starts at the end of the previous one. The reader reads
the chunks in order, as the client does. A chunk whose body ends after the end of the file is an
error. The reader does not search for a chunk tag. The client reads the same sequence and fails on
the same files.

### <a id="s6.2"></a>6.2 `BKHD` and `BankHeader`

```text
BKHD body
  u32       version             dwBankGeneratorVersion
  u32       bank_id             dwSoundBankID
  u32       language_id
  u16       alignment           uAlignment
  u16       alt_values          uAltValues
  u32       project_id
  -- present when version >= 143 --
  u32       bank_type           0 user, 30 event, 31 bus
  [u8; 16]  hash
  -- the rest of the body --
  [u8]      padding
```

```rust
/// The decoded `BKHD` chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankHeader {
    pub version: BankVersion,
    /// The ID in the header. The client does not read it.
    pub bank_id: BankId,
    pub language_id: u32,
    pub alignment: u16,
    pub alt_values: u16,
    pub project_id: u32,
    /// Zero below version 143.
    pub bank_type: BankType,
    /// Zero below version 143.
    pub hash: [u8; 16],
    /// The bytes of the body after the decoded fields.
    pub padding: Vec<u8>,
}

impl BankHeader {
    /// Returns a header with zero fields and no padding.
    pub fn new(version: BankVersion, bank_id: BankId) -> Self;
    /// Returns the alignment of media in `DATA`: `alignment` from version 135, else 16.
    /// An `alignment` of 0 gives 16.
    pub fn media_alignment(&self) -> u32;
}

/// A bank version: the `dwBankGeneratorVersion` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BankVersion(pub u32);

impl BankVersion {
    pub const V125: Self;
    pub const V132: Self;
    pub const V134: Self;
    pub const V145: Self;
    /// The version that the client reads as current.
    pub const CURRENT: Self;    // V145
    /// Returns `true` for 118 to 144.
    pub fn is_legacy(self) -> bool;
}

/// The `dwBankType` field.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BankType(pub u32);

impl BankType {
    pub const USER: Self;       // 0
    pub const EVENT: Self;      // 30
    pub const BUS: Self;        // 31
}
```

The writer writes the first 20 bytes, then `bank_type` and `hash` when `version` is 143 or
higher, then `padding`. The chunk size is the sum. A legacy bank ships a 24-byte body: 20 bytes
and 4 bytes of padding. A version 145 bank ships a 40-byte body with no padding.

The client reads `version` and `alignment`, and one bit of `alt_values`. The client does not read
`bank_id`, `language_id`, `project_id` or `hash`.

### <a id="s6.3"></a>6.3 `Bank` and `Section`

```rust
/// A bank read into memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bank { /* header, sections */ }

impl Bank {
    pub fn new(header: BankHeader) -> Self;
    pub fn from_reader(reader: &mut impl Read) -> Result<Self>;
    pub fn to_writer(&self, writer: &mut impl Write) -> Result<()>;

    pub fn header(&self) -> &BankHeader;
    pub fn header_mut(&mut self) -> &mut BankHeader;
    /// Returns the sections after `BKHD`, in file order.
    pub fn sections(&self) -> &[Section];
    pub fn sections_mut(&mut self) -> &mut Vec<Section>;
    /// Returns the first media index.
    pub fn media(&self) -> Option<&MediaIndex>;
    pub fn media_mut(&mut self) -> Option<&mut MediaIndex>;
    /// Returns the first hierarchy.
    pub fn hierarchy(&self) -> Option<&Hierarchy>;
    pub fn hierarchy_mut(&mut self) -> Option<&mut Hierarchy>;

    /// Returns the first condition under which the client rejects the bank.
    pub fn check(&self) -> std::result::Result<(), Rejection>;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSection {
    pub tag: [u8; 4],
    pub data: Vec<u8>,
}
```

A `DIDX` chunk followed by a `DATA` chunk is one `Section::Media`. A `DIDX` without a `DATA` after
it, and a `DATA` without a `DIDX` before it, are `Section::Raw`. A `HIRC` chunk is
`Section::Hierarchy`. Every other chunk is `Section::Raw`, in its file position. The writer writes
`BKHD`, then the sections in order. A bank that the caller does not edit writes back byte for
byte.

### <a id="s6.4"></a>6.4 Client acceptance

```rust
/// A condition under which the client rejects a bank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Rejection {
    /// The version is below 118 or above 145.
    Version(BankVersion),
    /// A legacy version and a chunk other than `DIDX` or `DATA`.
    LegacyChunk { version: BankVersion, tag: [u8; 4] },
    /// A bank type other than 0, 30 or 31.
    BankType(BankType),
}
```

`Bank::check` tests the conditions in the order of the client: version, bank type, then each
chunk in order. The retail client rejects such a bank with `AK_WrongBankVersion` and logs nothing.
`Bank::from_reader` does not call `check`. A tool reads and inspects a bank that the client
rejects.

## <a id="s7"></a>7. Media index

### <a id="s7.1"></a>7.1 Wire format (normative)

```text
DIDX body
  Record[size / 12]

Record
  u32       source_id
  u32       offset          from the start of the DATA body
  u32       size

DATA body
  [u8]      the media, each at its offset
```

The client honors `BankHeader::media_alignment` for media in `DATA`. Every shipped `DIDX` lists
its records in ascending source ID order ([appendix B](#appendix-b)).

### <a id="s7.2"></a>7.2 `MediaIndex`

```rust
/// The media of a bank, by source ID.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MediaIndex { /* media by source ID */ }

impl MediaIndex {
    pub fn new() -> Self;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn contains(&self, source: SourceId) -> bool;
    pub fn get(&self, source: SourceId) -> Option<&[u8]>;
    /// Returns the media in ascending source ID order.
    pub fn iter(&self) -> impl Iterator<Item = (SourceId, &[u8])>;
    /// Inserts `data` as the media `source`. Returns the media it replaces.
    pub fn insert(&mut self, source: SourceId, data: Vec<u8>) -> Option<Vec<u8>>;
    pub fn remove(&mut self, source: SourceId) -> Option<Vec<u8>>;
}
```

The reader rejects a record whose media ends after the end of `DATA`. The reader rejects two
records of one source ID. The reader accepts records in any order.

### <a id="s7.3"></a>7.3 Writing

A media index that the caller does not edit writes its `DIDX` and `DATA` bodies as read. An edited
media index writes:

- the records in ascending source ID order;
- each media at the first multiple of the media alignment of the bank at or after the end of the
  previous media, the first media at offset 0;
- zero bytes in each gap;
- a `DATA` body that ends at the end of the last media.

### <a id="s7.4"></a>7.4 Media-only bank

```rust
impl Bank {
    /// Returns a media bank for the file `name` that holds `media`.
    pub fn media_only(name: &str, media: MediaIndex) -> Self;
}
```

The header has version 145, the bank ID `BankId::from_name(name)`, alignment 16, bank type 0, zero
for every other field, and no padding. The sections are one `Section::Media`. The bank passes
`Bank::check`.

## <a id="s8"></a>8. Hierarchy

### <a id="s8.1"></a>8.1 Wire format (normative)

```text
HIRC body
  u32       object_count
  Object[object_count]

Object
  u8        kind
  u32       size
  [u8; size] payload        the first u32 is the object ID
```

The framing is the same at every version that the client reads. The body ends at the end of the
last object. A body with bytes after the last object is an error. A payload shorter than 4 bytes is
an error.

### <a id="s8.2"></a>8.2 `Hierarchy`, `Object` and `ObjectKind`

```rust
/// The objects of a `HIRC` chunk, in file order.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Hierarchy { /* objects */ }

impl Hierarchy {
    pub fn new() -> Self;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn objects(&self) -> &[Object];
    /// Returns the first object with the ID `id`.
    pub fn get(&self, id: ObjectId) -> Option<&Object>;
    pub fn get_mut(&mut self, id: ObjectId) -> Option<&mut Object>;
    /// Returns the objects of the kind `kind`, in file order.
    pub fn of_kind(&self, kind: ObjectKind) -> impl Iterator<Item = &Object>;
    pub fn push(&mut self, object: Object);
    /// Replaces the first object with the ID of `object` in place, or appends `object`.
    pub fn replace(&mut self, object: Object) -> Option<Object>;
    /// Removes the first object with the ID `id`.
    pub fn remove(&mut self, id: ObjectId) -> Option<Object>;
}

/// One hierarchy object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object { /* kind, payload */ }

impl Object {
    /// Returns an error when `payload` is shorter than 4 bytes.
    pub fn new(kind: ObjectKind, payload: Vec<u8>) -> Result<Self>;
    pub fn kind(&self) -> ObjectKind;
    pub fn id(&self) -> ObjectId;
    pub fn payload(&self) -> &[u8];
    pub fn into_payload(self) -> Vec<u8>;
}

/// The kind byte of a hierarchy object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectKind(pub u8);

impl ObjectKind {
    pub const STATE: Self;                              // 1
    pub const SOUND: Self;                              // 2
    pub const ACTION: Self;                             // 3
    pub const EVENT: Self;                              // 4
    pub const RANDOM_SEQUENCE_CONTAINER: Self;          // 5
    pub const SWITCH_CONTAINER: Self;                   // 6
    pub const ACTOR_MIXER: Self;                        // 7
    pub const BUS: Self;                                // 8
    pub const LAYER_CONTAINER: Self;                    // 9
    pub const MUSIC_SEGMENT: Self;                      // 10
    pub const MUSIC_TRACK: Self;                        // 11
    pub const MUSIC_SWITCH_CONTAINER: Self;             // 12
    pub const MUSIC_RANDOM_SEQUENCE_CONTAINER: Self;    // 13
    pub const ATTENUATION: Self;                        // 14
}
```

`ObjectKind` implements `Display` with the name of each constant, and `kind(N)` for any other
value. Kinds 15 to 22 exist in the version 145 reader. The crate names none of them.

The client function `SoundBank_ExtractEventIds` collects the IDs of kind 4 objects of a bank. It
reads only a bank whose path contains `_events` ([section 12](#s12)).

## <a id="s9"></a>9. Typed objects

### <a id="s9.1"></a>9.1 Layout generations

| Bank version | Generation |
| --- | --- |
| 132, 134 | legacy |
| 145 | current |
| any other | none: typed decoding returns `Error::UnsupportedLayout` |

Version 125 payloads do not close under these layouts. Single pad bytes occur between version 125
objects.

The grammar below is normative. `var` is a 7-bit big-endian varint: each byte contributes its low
7 bits, most significant group first, and a byte with bit 7 clear ends the value. Fields marked
**current** exist only in the current generation.

### <a id="s9.2"></a>9.2 Shared blocks

```text
PropBundle            u8 count; u8 id[count]; u32 value[count]
RangedPropBundle      u8 count; u8 id[count]; (u32 min, u32 max)[count]

Positioning
  u8        bits
  if bits & 1 and bits & 2:
    u8      bits_3d
    if (bits >> 5) & 3 in {1, 2}:
      u8    path_mode
      u32   transition_time
      u32   vertex_count;  (f32 x, f32 y, f32 z, i32 duration)[vertex_count]
      u32   item_count;    (u32 offset, f32 count)[item_count]
                           (u8 12-byte automation params)[item_count]

RtpcList
  u16       count
  per RTPC: u32 id; u8 type; u8 accum; var param_id; u32 curve_id; u8 scaling;
            u16 point_count; (f32 from, f32 to, u32 interp)[point_count]

StateChunk
  var       group_count;    per group: var group_id; u8 sync_type; u8 props
  var       prop_count;     per prop:  u32 id; u8 type; var value_count; (u32, u32)[value_count]

BankSource
  u32       plugin_id
  u8        stream_type     0 data bank, 1 prefetch, 2 streaming
  u32       source_id
  u32       in_memory_size
  u8        source_bits
  if plugin_id & 0x0F == 2:
    u32     param_size;     [u8; param_size]

ExceptionList         var count; (u32 id, u8 is_bus)[count]
```

### <a id="s9.3"></a>9.3 Node base

```text
NodeBase
  u8        override_parent_fx
  u8        fx_count
  if fx_count: u8 fx_bypass; (u8 index, u32 fx_id, u8 share_set, u8 rendered)[fx_count]
  current:  u8 override_parent_metadata
  current:  u8 metadata_count; (u8 index, u32 fx_id, u8 share_set)[metadata_count]
  u8        override_attachment_params
  u32       override_bus_id
  u32       direct_parent_id
  u8        bits
  PropBundle
  RangedPropBundle
  Positioning
  u8        aux_bits
  if aux_bits & 8: u32 aux_id[4]
  current:  u32 reflections_aux_bus
  u8        virtual_queue_behavior; u8 max_instances_bits; u16 max_instances; u8 below_threshold; u8 hdr_bits
  StateChunk
  RtpcList
```

```rust
/// The node base of a Sound, a container or an ActorMixer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeBase {
    pub override_bus: ObjectId,
    pub parent: ObjectId,
    /* the other fields of the grammar, kept as bytes */
}
```

### <a id="s9.4"></a>9.4 Sound

```text
Sound payload         u32 id; BankSource; NodeBase
```

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sound {
    pub id: ObjectId,
    pub source: BankSource,
    pub node: NodeBase,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankSource {
    pub plugin_id: u32,
    pub stream_type: StreamType,
    pub source_id: SourceId,
    pub in_memory_size: u32,
    pub source_bits: u8,
    /* plugin parameters, kept as bytes */
}

/// Where the media of a source is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StreamType(pub u8);

impl StreamType {
    /// The media is in the `DATA` of a loaded bank.
    pub const DATA_BANK: Self;      // 0
    /// The start of the media is in a bank. The rest streams from a package.
    pub const PREFETCH: Self;       // 1
    /// The media streams from a package.
    pub const STREAMING: Self;      // 2
}
```

### <a id="s9.5"></a>9.5 Action

```text
Action payload
  u32       id
  u16       action_type     the class is action_type & 0xFF00
  u32       target_id       idExt
  u8        target_flags    idExt_4
  PropBundle
  RangedPropBundle
  extension, by class:
    0x0100, 0x0200, 0x0300                u8; u8; ExceptionList
    0x0400 Play                           u8 fade_curve; u32 bank_id; current: u32
    0x0600, 0x0700, 0x2200                u8; ExceptionList
    0x0800 to 0x0F00, 0x2000, 0x3000      u8; u8; u32; u32; u32; ExceptionList
    0x1200, 0x1900                        u32; u32
    0x1300, 0x1400                        u8; u8; u8; u32; u32; u32; ExceptionList
    0x1A00, 0x1B00                        u8; u8; ExceptionList
    0x1E00                                u8; u32; u32; u32; u8; ExceptionList
    0x3100, 0x3200   current only         u8; u8; u32; u8; ExceptionList
    0x1500, 0x1600, 0x1700   legacy only  (nothing)
    0x1000, 0x1100, 0x1C00, 0x1D00, 0x1F00, 0x2100   (nothing)
```

A class outside the table is an error. A class of the other generation is an error.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub id: ObjectId,
    pub action_type: ActionType,
    pub target: ObjectId,
    pub target_flags: u8,
    /* the property bundles and the extension, kept as bytes */
}

impl Action {
    /// Returns a Play action of `target` in the bank `bank`, with empty property bundles and
    /// fade curve 4.
    pub fn play(id: ObjectId, target: ObjectId, bank: BankId) -> Self;
    /// Returns the bank of a Play action.
    pub fn play_bank(&self) -> Option<BankId>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActionType(pub u16);

impl ActionType {
    /// Play, `0x0403`.
    pub const PLAY: Self;
    /// Returns `action_type & 0xFF00`.
    pub fn class(self) -> u16;
    /// Returns `true` for the class `0x0400`.
    pub fn is_play(self) -> bool;
}
```

### <a id="s9.6"></a>9.6 Event

```text
Event payload         u32 id; var action_count; u32 action_id[action_count]
```

The action count is a varint in both generations.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub id: ObjectId,
    pub actions: Vec<ObjectId>,
}
```

### <a id="s9.7"></a>9.7 Containers

```text
RandomSequenceContainer payload (kind 5)
  u32 id; NodeBase
  u16; u16; u16; f32; f32; f32; u16; u8; u8; u8; u8
  u32 child_count; u32 child_id[child_count]
  u16 playlist_count; (u32 id, u32 weight)[playlist_count]

SwitchContainer payload (kind 6)
  u32 id; NodeBase
  u8 group_type; u32 group_id; u32 default_switch; u8 continuous_validation
  u32 child_count; u32 child_id[child_count]
  u32 group_count;  per group: u32 switch_id; u32 node_count; u32 node_id[node_count]
  u32 param_count;  (u32 node_id, u8, u8, f32, f32)[param_count]

ActorMixer payload (kind 7)
  u32 id; NodeBase
  u32 child_count; u32 child_id[child_count]

LayerContainer payload (kind 9)
  u32 id; NodeBase
  u32 child_count; u32 child_id[child_count]
  u32 layer_count;  per layer: u32 id; RtpcList; u32 rtpc_id; u8 rtpc_type;
                    u32 assoc_count; per association: u32 child_id; u32 point_count;
                                     (12 bytes)[point_count]
  u8 continuous_validation
```

```rust
/// A RandomSequenceContainer, a SwitchContainer, an ActorMixer or a LayerContainer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Container {
    pub id: ObjectId,
    pub kind: ObjectKind,
    pub node: NodeBase,
    pub children: Vec<ObjectId>,
    /* the fields before and after the children, kept as bytes */
}
```

A SwitchContainer and a LayerContainer name children again in their groups and layers. An edit of
`children` does not change those lists.

### <a id="s9.8"></a>9.8 Decoding and encoding

```rust
/// A decoded hierarchy object.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Typed {
    Event(Event),
    Action(Action),
    Sound(Sound),
    Container(Container),
}

impl Object {
    /// Decodes the object with the layouts of the generation of `version`.
    /// Returns `None` for a kind without a typed layout.
    pub fn decode(&self, version: BankVersion) -> Result<Option<Typed>>;
}

impl Typed {
    pub fn id(&self) -> ObjectId;
    /// Encodes the object with the layouts of the generation of `version`.
    pub fn encode(&self, version: BankVersion) -> Result<Object>;
}
```

`decode` succeeds only when the layout consumes the whole payload. Otherwise it returns
`Error::Layout` with the kind, the ID and the byte offset at which the layout stopped. Decoding
then encoding with the same version gives the original payload. Encoding with the other
generation writes zero into each **current** field and drops it for the legacy generation. A
caller edits an object by decoding it, changing a field, encoding it and passing it to
`Hierarchy::replace`. The other objects keep their bytes.

## <a id="s10"></a>10. Event sources

```rust
/// One media that an event plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventSource {
    pub sound: ObjectId,
    pub source: SourceId,
    pub stream_type: StreamType,
}

/// Returns the media that the event `event` plays, from the hierarchies of `banks`.
pub fn event_sources(banks: &[&Bank], event: ObjectId) -> Result<Vec<EventSource>>;
```

The query finds an object by ID in the banks in order and takes the first match. It decodes each
object with the version of the bank that holds it. From the event it follows each action of the
class Play to its target. A Sound gives one `EventSource`. A container gives the sources of its
children in order. Every other kind gives nothing. An object that no bank holds gives nothing. The
query visits each object once. The result is in the order of the walk, depth first. An event that
no bank holds gives an empty list. A decoding error stops the query and returns the error.

`stream_type` names the file that holds the media: a media index for `DATA_BANK`, a package for
`STREAMING`, both for `PREFETCH`.

## <a id="s11"></a>11. Upgrade

```rust
impl Bank {
    /// Returns the bank rewritten at version 145.
    pub fn upgrade(&self) -> Result<Bank>;
}

/// A reason the upgrade does not rewrite a bank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum UpgradeRefusal {
    /// The version is not 132, 134 or 145.
    Version(BankVersion),
    /// An object of a kind without an upgrade transform.
    Kind { id: ObjectId, kind: ObjectKind },
    /// An action of a class without a version 145 counterpart.
    ActionClass { id: ObjectId, class: u16 },
}
```

A version 145 bank upgrades to an equal bank. A version 132 or 134 bank upgrades as follows.

**Header.** Version 145. `alignment` 16 when it is 0. Bank type 0. Zero hash. No padding. The
other fields keep their values.

**Objects.**

| Kind | Transform |
| --- | --- |
| 1 State, 4 Event | none |
| 2 Sound, 5, 6, 7, 9 | node base: insert `00 00` after the FX block, insert a zero `u32` after the aux block |
| 3 Action | Play class: append a zero `u32`. Classes `0x1500`, `0x1600`, `0x1700`: refuse. Other classes: none |
| 14 Attenuation | insert a zero byte after the ID. Widen the curve table from 7 to 19 entries with `0xFE` |
| 8, 10 to 13, 15 to 22 | refuse |

Riot writes zero in each new node base field in every shipped version 145 object. In the version
145 reader `0xFE` in a curve slot selects the project default curve, and `0xFF` selects no curve.
The version 134 reader applies the project obstruction and occlusion curves to every attenuation.
The zero byte of an attenuation is the value of a zero-initialized version 134 object. 765 of 4000
shipped version 145 attenuations set it. The upgrade cannot recover it.

**Media.** A media index is unchanged. It is written with the alignment of the new header.

`upgrade` decodes every object of the output with the current layouts. A failure returns the
error and no bank. A refusal returns `Error::Upgrade`.

## <a id="s12"></a>12. Audio paths

```rust
/// A path of a `BankUnit`, classified by the rules of the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioPath<'a> { /* path */ }

impl<'a> AudioPath<'a> {
    pub fn new(path: &'a str) -> Self;
    pub fn kind(&self) -> AudioFileKind;
    /// Returns `true` when the client collects the event IDs of the bank.
    pub fn registers_events(&self) -> bool;
    /// Returns `true` when the client searches the localized path table first.
    pub fn is_localized(&self) -> bool;
    /// Returns the bank ID of the file name of the path.
    pub fn bank_id(&self) -> BankId;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioFileKind {
    Bank,
    Package,
}
```

- `kind` is `Package` when the path ends with `.wpk`, compared ASCII case-insensitively, and `Bank`
  otherwise. The client sends a `.wpk` path to `AudioPathTable_AddWpkEntries` and every other
  path to `BankManager_GetOrCreateBank`.
- `registers_events` is `true` when the path contains `_events`, compared ASCII
  case-insensitively.
- `is_localized` is `true` when the path contains `en_us`, compared ASCII case-insensitively. Four
  of the five loaders of the client test the path this way. The fifth,
  `BankUnitList_LoadAllUsingVoiceOver`, takes `BankUnit.voiceOver` instead.
- `bank_id` is `BankId::from_name` of the text after the last `/` or `\`.

The bin classes that hold bank paths are `SkinAudioProperties.bankUnits`,
`MapAudioDataProperties`, `FeatureAudioDataProperties`, `ClientStateAudioDataProperties.BankPaths`,
`SpellDataResource.AudioBankPaths` and `MapAlternateAsset.AudioBankUnits`. A caller reads them
through the tree API of `ltk_meta`. `ltk_meta` holds no schema (ADR-0006).

## <a id="s13"></a>13. WEM information

```rust
/// The format of a WEM, from its `fmt ` chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WemInfo {
    pub codec: WemCodec,
    pub channels: u16,
    pub sample_rate: u32,
    pub average_bytes_per_second: u32,
    /// The size of the `data` chunk.
    pub data_size: u32,
}

impl WemInfo {
    /// Reads the `fmt ` and `data` chunk headers of the RIFF file `data`.
    pub fn from_bytes(data: &[u8]) -> Result<Self>;
    /// Returns `data_size / average_bytes_per_second`, or `None` when the rate is 0.
    pub fn duration(&self) -> Option<std::time::Duration>;
}

/// The `wFormatTag` of a WEM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WemCodec(pub u16);

impl WemCodec {
    pub const PCM: Self;            // 0x0001
    pub const ADPCM: Self;          // 0x0002
    pub const OPUS: Self;           // 0x3040
    pub const WWISE_OPUS: Self;     // 0x3041
    pub const PCM_EXTENSIBLE: Self; // 0xFFFE
    pub const VORBIS: Self;         // 0xFFFF
}
```

`from_bytes` reads the RIFF header and walks the chunks until it has `fmt ` and `data`. Each chunk
body is padded to an even size. It does not read the audio. The duration is an estimate for the
compressed codecs.

## <a id="s14"></a>14. Errors

```rust
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// The first bytes are not the magic of the format.
    #[error("invalid magic: expected {expected:?}, found {found:?}")]
    InvalidMagic { expected: [u8; 4], found: [u8; 4] },
    #[error("unsupported package version {0}")]
    PackageVersion(u32),
    #[error("two package entries have the key {0:?}")]
    DuplicateKey(PackageKey),
    #[error("package entry {name} ends after the end of the package")]
    EntryOutOfBounds { name: String },
    /// The first chunk of a bank is not `BKHD`, or `BKHD` is shorter than 20 bytes.
    #[error("invalid bank header")]
    InvalidHeader,
    #[error("chunk {tag:?} at offset {offset} ends after the end of the bank")]
    ChunkOverrun { tag: [u8; 4], offset: u64 },
    #[error("media {source_id} ends after the end of DATA")]
    MediaOutOfBounds { source_id: SourceId },
    #[error("two media have the source ID {0}")]
    DuplicateSource(SourceId),
    /// The objects of `HIRC` do not end at the end of the chunk.
    #[error("hierarchy framing ends at {offset}, the chunk ends at {len}")]
    Framing { offset: usize, len: usize },
    #[error("object payload of {0} bytes is shorter than an ID")]
    PayloadTooShort(usize),
    #[error("no payload layouts for bank version {0:?}")]
    UnsupportedLayout(BankVersion),
    /// A layout did not consume the payload exactly.
    #[error("{kind} {id}: layout stops at byte {offset} of {len}")]
    Layout { kind: ObjectKind, id: ObjectId, offset: usize, len: usize },
    #[error("{id}: unknown action class {class:#06x}")]
    ActionClass { id: ObjectId, class: u16 },
    #[error("upgrade refused: {0:?}")]
    Upgrade(UpgradeRefusal),
    #[error("invalid WEM: {0}")]
    Wem(&'static str),
}
```

No function of the crate panics on input bytes.

## <a id="s15"></a>15. Testing

Tests drive the public surface: read, edit, write, read again, and assert on the second read. No
test calls a private layout function.

- **Fixtures.** `crates/ltk_wwise/tests/fixtures/` holds a few of the smallest shipped files:
  an empty bank, a version 145 media bank, a version 134 media bank with alignment 0, a version
  145 event bank, a version 134 event bank and a package. Each file is under 16 KiB. The builders
  of [section 5.3](#s5.3) and [section 7.4](#s7.4) produce the other inputs.
- **Corpus.** Tests marked `#[ignore]` read `LTK_LOL_GAME_DIR` and mount every WAD under
  `DATA/FINAL` with `ltk_wad`. Each chunk that starts with `BKHD` or `r3d2` is read and written
  back. The tests assert:
  - every package and every bank writes back byte for byte;
  - every package entry key equals the source ID of its name;
  - every `DIDX` lists its records in ascending source ID order;
  - every `HIRC` framing ends at the end of its chunk;
  - every object of a typed kind in a version 132, 134 or 145 bank decodes, and encodes back to
    its payload;
  - every legacy bank with a chunk other than `DIDX` or `DATA` fails `Bank::check`;
  - every version 132 and 134 bank without a refused kind upgrades.

  The tests print their counts. [Appendix B](#appendix-b) records them. `ltk_mapgeo`
  `write_corpus` is the model.
- **Hash.** `ltk_hash::fnv1` tests the attested bank IDs: `sett_base_sfx_audio` gives
  `0xE9B70B40`, `zed_skin11_sfx_audio.bnk` gives `0xDF17207F`, `aatrox_skin07_sfx_events.bnk`
  gives `0x962E8665`. One test shows that FNV-1a gives a different value.
- **Upgrade.** The version 134 event bank fixture upgrades, and the output decodes with the
  current layouts. A version 125 bank, a bank with a MusicSegment and a bank with a kind 17 object
  are refused.
- **Event sources.** The event bank fixture and its media bank resolve each event. An `insta`
  snapshot in `.ron` holds the result.

## <a id="s16"></a>16. Rules

| ID  | Rule | Instead of | Why | Spec |
| --- | ---- | ---------- | --- | ---- |
| W1  | One crate `ltk_wwise` holds packages, banks and the hierarchy. | Two crates `ltk_wpk` and `ltk_bnk`. A module of `ltk_wad`. | A `BankUnit` loads all three files. The event query reads them together. | [section 4](#s4); ADR-0031 |
| W2  | Wwise names hash with FNV-1, in `ltk_hash::fnv1`. | FNV-1a, the hash of bin names. | The client function `GetIDFromString` multiplies before it XORs. | [section 4.2](#s4.2) |
| W3  | The bank ID comes from the file name. The header `bank_id` is data. | Validating the header ID against the name. | The client registers a bank under the hash of the name and never reads the header ID. 18 shipped banks carry a different header ID. | [section 4.2](#s4.2), [section 6.2](#s6.2) |
| W4  | The reader reads chunks in sequence and fails on an overrun. | A rescan for the next tag. | The client reads in sequence. Nine shipped version 125 banks fail in the client. | [section 6.1](#s6.1) |
| W5  | Reading does not check client acceptance. `Bank::check` does. | Rejecting a legacy event bank in the reader. | A tool inspects and upgrades a bank that the client rejects. | [section 6.4](#s6.4) |
| W6  | An unedited bank, media index and package write back byte for byte. | Normalizing on write. | A mod changes only the bytes that it edits. | [section 5.5](#s5.5), [section 6.3](#s6.3), [section 7.3](#s7.3) |
| W7  | Every hierarchy object keeps its payload as bytes. Typed objects are decoded on request. | A typed enum for every kind in `Hierarchy`. | Kinds 8 and 10 to 22 have no checked layout. A raw payload round-trips every kind. | [section 8](#s8), [section 9.8](#s9.8) |
| W8  | `ObjectKind`, `BankVersion`, `BankType`, `StreamType`, `ActionType` and `WemCodec` are newtypes with constants. | Exhaustive enums. | A file carries values that the crate does not name. A Wwise upgrade adds values. | [section 6.2](#s6.2), [section 8.2](#s8.2) |
| W9  | A typed object decodes only when its layout consumes the whole payload. | Decoding the leading fields and ignoring the rest. | A layout error desynchronizes the client reader for every later object. | [section 9.8](#s9.8) |
| W10 | The upgrade fills new curve slots with `0xFE`. | `0xFF`. | `0xFF` disables obstruction, occlusion, diffraction and transmission in the version 145 reader. | [section 11](#s11) |
| W11 | The upgrade refuses music kinds, kinds 8 and 15 to 22, and version 125. | Passing them through. | The version 145 reader reads payloads in sequence. One payload of the old layout corrupts every later object. | [section 11](#s11) |
| W12 | A package holds one entry per key. | Keeping duplicate keys. | The client registers each entry under its key in one map. | [section 5.3](#s5.3) |
| W13 | `ltk_wwise` does not depend on `ltk_meta`. | A typed `BankUnit` in the crate. | `ltk_meta` holds no schema. | [section 12](#s12); ADR-0006 |
| W14 | The crate does not decode audio. | A Vorbis or Opus decoder. | The decoders are large dependencies. A mod replaces media with files that the Wwise tool encodes. | [section 13](#s13) |

## <a id="appendix-a"></a>Appendix A. Client functions

Retail 16.17.8057408. The names of Wwise internals are descriptive. The names under
`AK::SoundEngine` are exported symbols.

| Address | Name | Role |
| --- | --- | --- |
| `0x1416990A0` | `AkBankMgr_ReadBankHeader` | reads `BKHD`, applies the version gate |
| `0x141697140` | `AkBankMgr_ProcessBankChunks` | chunk loop, legacy `DIDX` and `DATA` rule |
| `0x1416616C0` | `AK_GetBankIDFromString` | bank ID from the file name |
| `0x141663150` | `AK::SoundEngine::LoadBank` | the overload by name that the client calls |
| `0x141699990` | HIRC dispatcher | switch on the object kind |
| `0x14167DAD0` | node base reader | seven callers |
| `0x14172F460` | Play action extension | reads the current `u32` |
| `0x1416F39F0` | Attenuation reader | 19 curve slots |
| `0x1403BD2D0` | `BankManager_LoadBankNow` | Riot, calls `LoadBank` |
| `0x1403BD560` | `SoundBank_ExtractEventIds` | Riot, collects kind 4 IDs of an `_events` bank |
| `0x1403BDDF0` | `AudioBankRequest_Build` | Riot, sends `.wpk` and `.bnk` to different tables |
| `0x1403BCED0` | `Wpk_ParseDirectory` | Riot, reads the package records and keys |
| `0x1403BDB40` | `BankUnitList_LoadAllUsingVoiceOver` | Riot, takes `BankUnit.voiceOver` |

The 2024 all-logs debug build holds the version 134 reader. Its HIRC dispatcher is at
`0x7FF6327321A0`.

## <a id="appendix-b"></a>Appendix B. Corpus measurements

**Retail 16.17, `DATA/FINAL`, from the `league_structs` notes.** 8875 banks.

| Version | Banks |
| --- | --- |
| 145 | 8016 |
| 134 | 839 |
| 125 | 18 |
| 132 | 2 |

22 banks are legacy and carry `HIRC`. The client rejects them. 18 banks carry a header ID that
differs from the hash of their name. Every bank carries a nonzero header ID. 35,428 objects of
kinds 1 to 7, 9 and 14 close exactly under the layouts of [section 9](#s9).

**Retail 16.x, five WADs (`Ahri`, `Zed`, `Sett`, `Global`, `Map11`), 2026-10-28.** 679 media
banks. Every `DIDX` lists its records in ascending source ID order. 475 are version 145 with
alignment 16. 204 are version 134 with alignment 0. Each media offset is a multiple of 16.

**Retail 16.x, `Ahri.en_US.wad.client`, 2026-10-28.** Every package is `r3d2` version 1. Entry
records and media start on 8-byte boundaries. The data region starts at the end of the last entry
record. The file ends at the end of the last media.
