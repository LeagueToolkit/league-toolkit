# PTCH property patches and property paths in `ltk_meta`

The API spec for patch bins in `ltk_meta`: the wire format, the types, the path language, and the
operations a consumer runs over them - apply, merge, diff and join.

**This document states what is true now.** Where the code and a section here disagree, the section
is the bug and gets edited. Two things it does not hold:

- **Why the feature exists, who asks for it, and what it must do** -
  `docs/prd/001-ptch-property-patches.md`, cited here as FR-N.
- **Why an option was chosen over the alternatives it beat** - `docs/adr/0001` to `0006`, cited as
  ADR-NNNN from the rules in [section 17](#s17).

Implemented today: [section 4](#s4) to [section 9](#s9) - reading, writing, the path language,
resolution and apply. Designed and not yet built: [section 10](#s10) to [section 14](#s14) (merge,
`ValuePath`, diff, join, the per-record surface), tracked as #219 to #223, and [section 15](#s15)
(ritobin text).

## <a id="s1"></a>1. Summary

A `PTCH` file is a patch applied over exactly one base `PROP` bin. After a short header it carries
three things: a set of object hashes to delete, whole objects to add, and a list of property patch
records. Each record names one object of the base by hash, one property inside that object by a
typed path string (`Position.UIRect.Size`, `Elements[3]`, `PerAttachmentMaterial{"weapon"}`), and a
value. The client resolves the path against its reflection tables and writes the value in place.

Riot ships 238 of these files, all UI scene variants - flipped minimap, mobile and tablet layouts,
mirrored scoreboard - carrying 23,047 records against 582 whole objects.

`ltk_meta` reads and writes them, resolves a path against a value tree, and applies a patch over a
base with the client's skip semantics. Above that sit three operations a mod manager needs and the
client has no counterpart for: **merge** layers one bin over another without passing through the
record language, **diff** renders the difference between two bins as records wherever records can
carry it, and **join** concatenates several patches over one target and reports the collisions.

What a consumer could not do before this, and the larger problem behind merge, are PRD-001
[section 1](../prd/001-ptch-property-patches.md#s1). The load-bearing non-goal is that no class
schema enters this crate: everything here works on the serialized value tree, never on Riot's meta
classes (ADR-0006).

## <a id="s2"></a>2. Vocabulary

Every term this document uses in a specific sense. Where a name was contested, the ADR beside it
holds the argument; the definition lives here.

**The file and its parts**

- **patch bin** - a `PTCH` file. Riot's loader calls it a data override
  (`BinFileCache_addDataOverride`, `PropertyOverrideLoadable`, `cache->overrides`), which is where
  the type name `BinOverride` comes from. The magic, ritobin and LtMAO call it a patch, which is
  what this document calls it in prose (ADR-0001).
- **patch record** - one entry of the record list: an object hash, a property path, a value. The
  type is `PropertyPatch` and the per-property verb is `patch()`.
- **base** - the `PROP` bin a patch applies over. Exactly one; a patch's reach never extends past
  it.
- **layer** - a verb, never a noun for this file or any part of it. `ltk-manager` ADR-0012 defines
  its overlay build as layering a mod's content over the game's copy, and that is the sense used
  here. The reversing notes' noun "layer" means the client's cache entry attaching a patch to a
  base, which is a different thing and has no name in this crate. Where a position among several
  overrides is needed the field is `overrides`: `override` alone is reserved in Rust, the plural is
  not (ADR-0001).
- **wildcard patch** - a patch whose target hash is 0, which the client offers to every bin it
  parses. What is attested about them and what is not is
  PRD-001 [section 5](../prd/001-ptch-property-patches.md#s5).

**Classes and schemas** (ADR-0006)

- **class** - the hash a bin object carries.
- **meta class** - one class's dumped definition: its properties, their types, its base chain, its
  default values. Riot's own word, and the `lol-meta-classes` repository's.
- **schema** - the collection of meta classes for one build. No schema enters this crate.

**The operations**

- **apply** - run a patch's records over a base with the client's semantics: a record that does not
  fit is skipped and nothing is fatal ([section 9.5](#s9.5)).
- **merge** - layer one bin over another in process, field by field and key by key, under no
  serialization constraint. It can express things no record can, which is why it is not apply
  ([section 10](#s10)).
- **diff** - render the difference between two bins as a patch bin, escalating where a record
  cannot carry what the walk found ([section 12](#s12)).
- **join** - concatenate several patches over one target, reporting collisions ([section 13](#s13)).
- **skip** and **escalate** - a skip is a record that did not apply and changed nothing. An
  escalation is a diff emitting a coarser record than the difference it found - a whole map instead
  of the one key that changed - which loses precision but stays expressible.

**Addresses**

- **`PropertyPath`** - the client's path language, as text. What a record carries and what the
  client resolves ([section 8](#s8)).
- **`ValuePath`** - this crate's address for a position in a value tree, by hash and by position.
  Total, where a `PropertyPath` is not: container elements and map entries have no name. Never
  written to a file ([section 11](#s11), ADR-0005).

## <a id="s3"></a>3. Evidence

Two sources, plus a scan of the installed client:

- **The client's bin loader, decompiled.** Record layout, the version gates, the delete list,
  dependency handling, the type rule, the resolver and the apply loop. [appendix A](#appendix-a)
  lists the functions this rests on, with addresses for a 16.14-era and a 16.16 build; the
  statistics behind it came from 16.14 and 16.15 clients.
- **`PropertyPath.hpp`.** `PropertyManager::ResolvePropertyString` and `PropertyPathIterator`,
  which pin the path grammar and the `{key}` subscript as JSON text.

### <a id="s3.1"></a>3.1 Verified on the installed client

Client 16.16.804.9184, `UI.wad.client`, scanned on 2026-08-22 with a scratch reader built on
`ltk_wad` and `ltk_meta` (not committed):

| measurement                                                                                         | result                                                                                                                                                     |
| --------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| PTCH chunks                                                                                         | 237 (the 238th lives in `Bootstrap.windows.wad.client`)                                                                                                    |
| parse with the layout in [section 4](#s4), to the exact end of the chunk                                   | 237 of 237, zero trailing bytes                                                                                                                            |
| records / whole objects                                                                             | 22,899 / 582                                                                                                                                               |
| `payloadSize` equals `1 + 2 + pathLen + value size`, value read by `PropertyValueEnum::from_reader` | 22,899 of 22,899                                                                                                                                           |
| outer version 1, inner version 3, delete count 0, dependency count 0                                | all 237                                                                                                                                                    |
| paths containing `{`                                                                                | 0; longest path 48 bytes; all ASCII                                                                                                                        |
| record kinds                                                                                        | Link 3911, Vec2 3642, Embed 3495, Pointer 2885, Bool 2695, U16 2511, U32 1813, String 616, U8 417, Vec4 356, F32 325, Color 151, List2 80, Map 1, Option 1 |

Every record was then resolved against the `uibase` bin of the same directory, walking
`ltk_meta`'s value tree with the rules in [section 9](#s9):

| outcome                                                                       | records | meaning                                                                                                                                                                                                                           |
| ----------------------------------------------------------------------------- | ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| leaf exists, same shape                                                       | 20,203  | plain replace                                                                                                                                                                                                                     |
| leaf absent, parent exists                                                    | 2,455   | the base object omits the property (class default). The client still applies the record because the property exists on the class, so apply must insert at the leaf.                                                               |
| object absent from base and patch                                             | 173     | the client skips (binary-search miss). Stale records, or a variant patched over a base other than `uibase`.                                                                                                                       |
| intermediate segment absent                                                   | 68      | for example `ClickedStateElements.DisplayElementList` when the base object omits `ClickedStateElements`, an Embed. The client applies into the default-constructed embedded struct; without the class we cannot. Skip and report. |
| kind mismatch, index out of range, subscript on a non-container, null pointer | 0       |                                                                                                                                                                                                                                   |

No shipped patch object collides with a base object, so replace-on-collision is unattested either
way.

### <a id="s3.2"></a>3.2 Worked example

Record 0 of `clientstates/gameplay/ux/lol/lolminimap/uiflipped` (5762 bytes, 0 objects,
109 records):

```text
14 c4 47 4a               objectHash   0x4a47c414  ...LoLMinimap/UIBase/Minimap/VoiceChatButton/VoiceChatPanel_ButtonClicked
22 00 00 00               payloadSize  34
0b                        kind         0x0b Vector2
17 00                     pathLen      23
50 6f 73 .. 68 6f 72      path         "Position.Anchors.Anchor"
00 00 00 00 00 00 80 3f   value        Vec2(0.0, 1.0)
```

In `uibase`, object `0x4a47c414` is a `UiElementIconData`. `Position` is a Pointer to
`UiPositionRect`, `Anchors` a Pointer to `AnchorSingle`, `Anchor` a `Vector2` holding `(1.0, 1.0)`.
The record re-anchors the element from the right edge to the left. Record 1 replaces the whole
`Position.UIRect` Embed (`UiElementRect`, 4 properties) in one value. Record 2 sets `FlipX` (Bool)
on `MinimapFrame`, a property that object does not serialize in `uibase` at all: the insert case.

## <a id="s4"></a>4. Wire format (normative)

All integers are little-endian.

```text
PTCH patch
  'PTCH'                    u8[4]
  version                   u32           must be 1
  deleteCount               u32
  deleted[deleteCount]      u32           object path hashes; the client drops every object with one of
                                          these hashes, from the base and from the patch alike
  'PROP'                    u8[4]
  version                   u32           must be 3
  dependencyCount           u32           must be 0 (D3)
  objectCount               u32
  classHash[objectCount]    u32
  object[objectCount]                     same encoding as a PROP object:
                                          u32 size, u32 pathHash, u16 propertyCount, properties
  patchCount                u32
  record[patchCount]
      objectHash            u32           path hash of a top-level object in the merged (base + patch) table
      payloadSize           u32           number of bytes that follow this field in the record
      kind                  u8            bin type tag of the value
      pathLen               u16
      path                  u8[pathLen]   the property path, not NUL-terminated
      value                               encoded exactly like a property value body of `kind`
                                          (no name hash, no kind byte)
```

Client constraints that decide whether a file loads at all, all read off the loader functions in
[appendix A](#appendix-a):

- Outer version exactly 1, inner version exactly 3; unchanged across 75 builds from 15.19 to 16.17.
- `dependencyCount` must be 0: the client reads the count and never skips the strings behind it,
  so any non-zero value desyncs the loader.
- The delete list is tested against every object read, base and patches.
- `size` and `payloadSize` are used only to skip; counts drive parsing.
- A record is skipped when `objectHash` is not in the merged table, when the path does not
  resolve, or when `kind` differs from the resolved property's registered tag; skipping is silent
  and the load continues.
- A PTCH is only ever a patch: it is never loaded as a base file and cannot be pulled in through
  a `linked` entry.

## <a id="s5"></a>5. Data model

Module layout in `crates/ltk_meta/src`:

```text
lib.rs                    pub mod path; mod data_override; mod file; explicit re-exports
path.rs                   PropertyPath, Segment, Segments, Subscript, KeyLiteral, PropertyPathError
path/parse.rs             tokenizer
path/resolve.rs           resolver, ResolveError, PatchError, ValueShape
data_override.rs          BinOverride, BinOverrideBuilder, PropertyPatch
data_override/read.rs     from_reader
data_override/write.rs    to_writer
data_override/apply.rs    apply, check, ApplyReport
file.rs                   BinFile, BinKind
tree/read.rs              object-table reader shared by Bin and BinOverride
```

Visibility follows M-SINGLE-ITEM-PATH: `data_override` and `file` are private modules whose types
are re-exported from the crate root, the same arrangement as `tree`; `path` is a public module and
its items are not re-exported.

### <a id="s5.1"></a>5.1 `BinOverride`

```rust
/// A `PTCH` patch: deletions, added objects and property patch records applied over one base bin.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(bound = "..."))]
pub struct BinOverride<M = NoMeta> {
    /// Object hashes the client drops from the base and from this patch.
    pub deleted: Vec<BinHash>,
    /// Whole objects this patch adds, keyed by path hash.
    pub objects: IndexMap<BinHash, BinObject<M>>,
    /// Property patches in file order. Applied after `deleted` and `objects`.
    pub patches: Vec<PropertyPatch<M>>,
}

impl<M> BinOverride<M> {
    pub fn new() -> Self;
    pub fn builder() -> BinOverrideBuilder<M>;
    /// True when the patch changes nothing. 38 of Riot's 238 files are inert.
    pub fn is_empty(&self) -> bool;
}
impl BinOverride {
    /// Reads a `BinOverride<NoMeta>`.
    ///
    /// Not generic over `M`: Rust does not apply a struct's default type parameter in
    /// expression position, so an `impl<M: Default>` block cannot infer `M` here and every
    /// call site would need a turbofish. `Bin::from_reader` and `BinFile::from_reader` have
    /// the same shape for the same reason.
    pub fn from_reader<R: Read + Seek + ?Sized>(reader: &mut R) -> Result<Self, Error>;
}
impl<M: Clone> BinOverride<M> {
    pub fn to_writer<W: Write + Seek + ?Sized>(&self, writer: &mut W) -> io::Result<()>;
}
impl<M> BinOverride<M> {
    /// Consumes the patch: objects and values move into the base, nothing is cloned (D17).
    pub fn apply(self, base: &mut Bin<M>) -> ApplyReport;
    pub fn check(&self, base: &Bin<M>) -> ApplyReport;
}
```

No version fields: the client accepts exactly one value for each (D2); an older inner version is
read and written back as 3, as `Bin` does (D15). No dependencies: the client cannot
load a patch that declares any (D3). `deleted` is a `Vec`, not a set, so a file round-trips
byte-for-byte. Fields are public for parity with `Bin` and `BinObject`. `Default` is implemented for
`BinOverride<NoMeta>` only, for the same inference reason as `from_reader`; `new()` stays on
`impl<M>`.

The builder follows C-BUILDER and M-INIT-BUILDER (named `BinOverrideBuilder`, chainable, `build()`
last, no public `new`):

```rust
let patch_bin = BinOverride::<NoMeta>::builder()
    .delete(0xdeadbeef)
    .object(BinObject::new(0x1234, 0x5678))
    .set(0x4a47c414, PropertyPath::new("Position.Anchors.Anchor")?, values::Vector2::new(Vec2::new(0.0, 1.0)))
    .patch(PropertyPatch::new(0xa4edcb0d, PropertyPath::new("FlipX")?, values::Bool::new(true)))
    .build();
```

Bulk setters sit beside the single-item ones: `objects`, `patches` and `deletions`.

`build()` is infallible: the only fallible piece, `PropertyPath::new`, is validated before it
enters the builder, so M-BUILD-RESULT has nothing left to check.

### <a id="s5.2"></a>5.2 `PropertyPatch`

```rust
/// One record: set the property at `path` inside object `object_hash` to `value`.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyPatch<M = NoMeta> {
    pub object_hash: BinHash,
    pub path: PropertyPath,
    pub value: PropertyValueEnum<M>,
}

impl<M> PropertyPatch<M> {
    pub fn new(object_hash: impl Into<BinHash>, path: PropertyPath, value: impl Into<PropertyValueEnum<M>>) -> Self;
    /// The wire kind tag, always `self.value.kind()`.
    pub fn kind(&self) -> Kind;
}
```

The kind byte is not stored separately. It is always `value.kind()`, so the two cannot disagree.

### <a id="s5.3"></a>5.3 `PropertyPath`

A validated newtype over `String` (C-NEWTYPE, M-STRONG-TYPES-GUARD): every `PropertyPath` is
well-formed per [section 8](#s8). The text is preserved byte-for-byte (casing, `0x` indices,
whitespace inside a key), so a read file round-trips exactly. `PartialEq`, `Hash` and `Ord` are
textual; use `segments()` and `Segment::name_hash` for a case-insensitive comparison.

```rust
pub struct PropertyPath(String);

impl PropertyPath {
    /// Wire limit: `pathLen` is a `u16`.
    pub const MAX_LEN: usize = u16::MAX as usize;

    pub fn new(path: impl Into<String>) -> Result<Self, PropertyPathError>;
    pub fn as_str(&self) -> &str;
    pub fn segments(&self) -> Segments<'_>;   // Iterator<Item = Segment<'_>> + Clone
    pub fn len(&self) -> usize;               // segments, not bytes
    pub fn is_empty(&self) -> bool;

    // In-place extension like `PathBuf::push`; each call validates the new piece.
    // `push_field` rejects a name that is not exactly one segment: `push_field("A.B")`
    // would append two, which is not what the method says it does.
    pub fn push_field(&mut self, name: &str) -> Result<(), PropertyPathError>;
    pub fn push_index(&mut self, index: u32) -> Result<(), PropertyPathError>;      // renders decimal
    pub fn push_key(&mut self, key: &KeyLiteral<'_>) -> Result<(), PropertyPathError>; // renders JSON
}
// Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Display (the text), FromStr,
// AsRef<str>, Borrow<str>, TryFrom<&str>, TryFrom<String>, From<PropertyPath> for String,
// serde: a plain string, validated on deserialize.

pub struct Segment<'a> {
    pub name: &'a str,
    pub subscript: Option<Subscript<'a>>,
}
impl Segment<'_> {
    /// FNV-1a of the ASCII-lowercased name, i.e. `BinHash::hash_str`.
    pub fn name_hash(&self) -> BinHash;
}
pub enum Subscript<'a> { Index(u32), Key(KeyLiteral<'a>) }
/// The JSON scalar inside `{...}`. `Number` keeps the validated text; `String` is unescaped.
pub enum KeyLiteral<'a> { Bool(bool), Number(&'a str), String(Cow<'a, str>) }
```

There is no `From<&str>`: construction is fallible, so only `TryFrom` and `FromStr` exist
(M-STRONG-TYPES-GUARD). `Segment`, `Subscript` and `KeyLiteral` implement `Display` in path syntax
(`Elements[3]`, `Lookup{"weapon"}`), which is what `push_*` writes.

The type lives in `ltk_meta::path`, not under `data_override`, because the same language is used by
Riot's tools for deep pathing and by some property values at runtime, and #173 asks for it on
regular bin objects. The patch record is one consumer.

### <a id="s5.4"></a>5.4 `BinFile` and `BinKind`

A `.bin` extension does not say which kind a file is, so the crate answers that question two ways:
read it as a `BinFile` and match on what came back, or ask `BinKind` and call the reader you want.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinKind { Prop, Override }
impl BinKind {
    pub const fn magic(self) -> [u8; 4];                       // b"PROP" / b"PTCH"
    pub fn from_magic(magic: [u8; 4]) -> Option<Self>;
    /// From a buffer, e.g. a decompressed wad chunk. Reads no further than the magic.
    pub fn identify_from_bytes(data: &[u8]) -> Option<Self>;
    /// Leaves the reader where it was, so `Bin::from_reader` / `BinOverride::from_reader`
    /// can be handed the same reader.
    pub fn identify_from_reader<R: Read + Seek + ?Sized>(reader: &mut R) -> Result<Self, Error>;
}

pub enum BinFile<M = NoMeta> { Prop(Bin<M>), Override(BinOverride<M>) }
impl BinFile {
    /// Sniffs the magic and reads the matching type.
    pub fn from_reader<R: Read + Seek + ?Sized>(reader: &mut R) -> Result<Self, Error>;
}
impl<M: Clone> BinFile<M> {
    pub fn to_writer<W: Write + Seek + ?Sized>(&self, writer: &mut W) -> io::Result<()>;
}
impl<M> BinFile<M> {
    pub fn kind(&self) -> BinKind;
    pub fn is_prop(&self) -> bool;
    pub fn is_override(&self) -> bool;
    pub fn as_prop(&self) -> Option<&Bin<M>>;                       // + _mut, into_
    pub fn as_override(&self) -> Option<&BinOverride<M>>;           // + _mut, into_
    /// The object table both kinds carry. For a `PTCH`, the objects it adds.
    pub fn objects(&self) -> &IndexMap<BinHash, BinObject<M>>;      // + _mut
}
// From<Bin<M>> and From<BinOverride<M>>.
```

Replaces `Bin::PROP` / `Bin::PTCH`. Tools that scan archives (bin-grep, extractors, the LSP) get one
entry point instead of sniffing the magic themselves (D12).

### <a id="s5.5"></a>5.5 Container storage and `ValueSlot`

[section 9.1](#s9.1) hands out `&PropertyValueEnum<M>`, so `Container`, `UnorderedContainer` and
`Optional` store `Vec<PropertyValueEnum<M>>` beside an `item_kind: Kind`, the arrangement `Map` has
always had. `Optional` boxes its single value, because the type allows a nesting the format forbids.
The homogeneity that typed variants (`Vec<values::Vector2<M>>`) once enforced at compile time is
enforced at run time by the constructors and by `push`; `ContainerItem`, a marker for the value
types the format lets a container hold, keeps `From` and `FromIterator` rejecting a nested container
at compile time.

That storage opens a hole the typed variants closed: a `&mut PropertyValueEnum` reaching into a
container could be assigned a value of another kind, leaving the container disagreeing with its own
declared item kind and the writer emitting, silently, a file the game cannot read.

`ValueSlot` closes it, and it is why **no `&mut PropertyValueEnum` is handed out anywhere in this
crate**. A mutable borrow is used for two different things and only one of them is dangerous:
replacing the whole value can change the kind, editing inside it cannot. So `Container::items_mut`
and `Optional::value_mut` are replaced by `slot`, which returns a handle carrying the kind its
holder pins it to. `ValueSlot::set` checks that kind; `ValueSlot::as_mut` and `ValueSlot::get_mut`
reach the concrete value type, where the kind is not expressible as anything else. A slot on an
object or struct property pins nothing, because there the kind is free.

Two supporting pieces on `PropertyValueEnum` fall out of this and are useful on their own, both
generated from the existing variant list: `ValueMut`, a borrowed enum with one variant per kind,
and `FromValue`, which sits behind `get` and `get_mut`. Before them the crate had no `as_*`
accessors at all, so reaching an `i32` meant writing a `match`.

## <a id="s6"></a>6. `Bin` and the shared reader

`Bin` is the `PROP` type only. It carries no `is_override` flag and no `data_overrides` field, and
`Builder::is_override` does not exist. `Bin::from_reader` on a `PTCH` magic returns
`Error::UnexpectedBinKind { expected: Prop, found: Override }` rather than reading half the file,
and `Bin::to_writer` always writes `PROP`. The magic constants `Bin::PROP` and `Bin::PTCH` are
`BinKind::magic()`.

The object-table reader is factored out of `Bin::from_reader` so `BinOverride` shares it, including
the legacy-kind retry (D6).

Dropping the flag was breaking, in `ltk_meta` 0.7.0. Known downstream use to migrate:
`crauzer-ritobin-lsp` reads `bin.is_override` in three places.

Variants on `ltk_meta::Error`:

```rust
#[error("expected a {expected} bin, found a {found} bin")]
UnexpectedBinKind { expected: BinKind, found: BinKind },
#[error("unsupported PTCH version {0}, the client accepts only 1")]
InvalidOverrideVersion(u32),
#[error("the PTCH's inner PROP declares {0} dependencies, the client cannot load a patch that has any")]
OverrideDependencies(u32),
#[error("invalid property path in patch record {index} (object {object_hash:08x}): {source}")]
InvalidPropertyPath { index: usize, object_hash: BinHash, #[source] source: PropertyPathError },
#[error(transparent)]
Resolve(#[from] ResolveError),   // so `?` works in code that mixes I/O and resolution
```

`Error` is `#[non_exhaustive]`, and so are the three error enums this document introduces:
`PropertyPathErrorKind` ([section 8.3](#s8.3)), `ResolveErrorKind` and `PatchError`
([section 9.4](#s9.4)). The streaming work adds variants to them in minor releases, so a downstream
match needs a wildcard arm; crate-internal exhaustive matches still compile.

## <a id="s7"></a>7. Reading and writing rules

`BinOverride::from_reader`:

1. Magic `PTCH`. A `PROP` magic gives `UnexpectedBinKind`; anything else `InvalidFileSignature`.
2. Outer version must be 1, else `InvalidOverrideVersion`.
3. `deleteCount` hashes are read into `deleted`. This is the header bug fix.
4. Inner magic must be `PROP` (`InvalidFileSignature`). Inner versions 1 to 3 are accepted, as
   for `Bin` (D15): the dependency count exists from version 2 and the record list only in
   version 3, the gate LtMAO uses too. Anything else is `InvalidFileVersion`.
5. `dependencyCount` (version 2 and up) must be 0, else `OverrideDependencies` (D3).
6. Objects go through the reader shared with `Bin`: class table, sized objects, `InvalidSize`
   enforcement, legacy retry (D6). The retry rewinds to the position recorded before the object
   table: re-reading from wherever the first attempt failed would land mid-object.
7. Records: `objectHash`, `payloadSize`, then the body under `ltk_io_ext::measure`, compared to
   `payloadSize` and rejected with `InvalidSize` on mismatch (the toolkit enforces the sizes the
   client ignores, consistent with objects, structs and maps). `kind` goes through
   `Kind::unpack(raw, legacy)`, the path through `read_sized_string_u16` and `PropertyPath::new`
   (`InvalidPropertyPath`, D4), the value through `PropertyValueEnum::from_reader(reader, kind,
   legacy)`.
8. No end-of-stream check, same as `Bin`.

`BinOverride::to_writer` always emits `PTCH`, `1`, `deleted`, `PROP`, `3`, `0`, the class table and
objects exactly as `Bin` does, `patches.len()`, then per record `objectHash`, a size placeholder,
the body under `measure`, and the size back-patched with `window_at`. For every shipped file the
output is byte-identical to the input ([section 16](#s16)).

## <a id="s8"></a>8. Property path grammar

### <a id="s8.1"></a>8.1 Grammar (normative for `PropertyPath::new`)

```text
path       = segment *( "." segment )
segment    = name [ subscript ]
name       = 1*name-byte        ; any byte except "." "[" "]" "{" "}" "(" ")" and control characters
subscript  = "[" index "]" / "{" key "}"
index      = dec-int / "0x" hex-int / "0" oct-int
             ; strtol(base 0): no sign, no whitespace, must fill the brackets
key        = [ws] ( json-number / json-string / "true" / "false" ) [ws]
```

Semantics:

- `name` matches a property by `FNV1a32(lowercase(name))`, which is `BinHash::hash_str`.
  Lowercasing is ASCII-only in the client (`c - 'A' <= 25 ? c + 32 : c`) and in
  `ltk_hash::fnv1a::hash_lower` for ASCII input. Casing in the text is cosmetic.
- `[index]` selects element `index` of a List or List2. On an Option it selects the contained
  value and only `[0]` can succeed (D9).
- `{key}` selects the Map entry whose key equals `key` converted to the map's declared key kind.
  The key text is JSON: `{5}`, `{"weapon"}`, `{true}`. For hash-typed keys a string is hashed
  (`Hash` with FNV-1a lower, `WadChunkLink` with XXH64 lower) and a number is the raw value. This
  is inferred from `PropertyPath.hpp` (the parsed JSON variant is coerced to the key POD type
  through `CallPropertyFunctorPOD`); no shipped record exercises it (D10).
- Moving from one segment to the next depends on the value reached: a Pointer (0x82) is
  dereferenced and a class hash of 0 is null, an Embed (0x83) is descended inline, everything
  else is a leaf or is only indexable.

| path                                             | parses to                     | note                                                           |
| ------------------------------------------------ | ----------------------------- | -------------------------------------------------------------- |
| `Enabled`                                        | field                         |                                                                |
| `Position.UIRect.Size`                           | field, field, field           |                                                                |
| `Elements[3]`                                    | field + Index(3)              |                                                                |
| `AnimationItems[0x1].SpeedScale`                 | Index(1), then field          | hex accepted, text preserved                                   |
| `PerAttachmentMaterial{"weapon"}`                | field + Key(String("weapon")) |                                                                |
| `Lookup{ 12 }`                                   | field + Key(Number("12"))     | JSON whitespace allowed around a key                           |
| (empty string)                                   | error `EmptySegment`          |                                                                |
| `Position.` or `.Position`                       | `EmptySegment`                |                                                                |
| `Elements[3]x`                                   | `UnexpectedCharacter('x')`    | a subscript must end the segment                               |
| `Elements[-1]`, `Elements[ 3 ]`, `Elements[3.0]` | `InvalidIndex`                |                                                                |
| `Map{null}`, `Map{[1]}`                          | `InvalidKey`                  | only scalars can convert to a key kind                         |
| `A[1][2]`, `A[1]{2}`                             | `DoubleSubscript`             | one subscript per segment; the format has no nested containers |
| `A[(1]`                                          | `UnbalancedBracket`           |                                                                |

### <a id="s8.2"></a>8.2 Where we are stricter than the client

The client's tokenizer never rejects a path; bad paths fail at resolution and the record is skipped.
`PropertyPath::new` rejects: unbalanced brackets, empty names, parentheses and control characters in
names (the client only counts parentheses for nesting), indices that are not a complete
non-negative `strtol` token, keys that are not a JSON scalar, and paths longer than `u16::MAX`.
Every such path is dead weight the game would silently ignore, and the toolkit already rejects
other things the client tolerates (size mismatches). All 23,047 shipped records pass.

The path is copied into a 512-byte stack buffer, but the retail limit is not measured and shipped
paths top out at 48 bytes, so the limit is documented, not enforced.

### <a id="s8.3"></a>8.3 `PropertyPathError`

```rust
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{kind} at byte {offset}")]
pub struct PropertyPathError { offset: usize, kind: PropertyPathErrorKind }
impl PropertyPathError {
    pub fn offset(&self) -> usize;
    pub fn kind(&self) -> PropertyPathErrorKind;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PropertyPathErrorKind {
    EmptySegment,
    UnexpectedCharacter(char),
    UnbalancedBracket,
    DoubleSubscript,
    InvalidIndex,
    InvalidKey,
    TooLong(usize),
}
```

## <a id="s9"></a>9. Resolution and apply

### <a id="s9.1"></a>9.1 API

```rust
impl<M> BinObject<M> {
    pub fn resolve(&self, path: &PropertyPath) -> Result<&PropertyValueEnum<M>, ResolveError>;
    pub fn resolve_mut(&mut self, path: &PropertyPath) -> Result<ValueSlot<'_, M>, ResolveError>;
    /// Client semantics: the type rule, then replace or insert at the leaf.
    /// Returns the replaced value, `None` when the leaf was inserted.
    pub fn patch(&mut self, path: &PropertyPath, value: PropertyValueEnum<M>)
        -> Result<Option<PropertyValueEnum<M>>, PatchError>;
}
impl<M> values::Struct<M>   { /* the same three */ }
impl<M> values::Embedded<M> { /* the same three, forwarding to .0 */ }
impl<M> PropertyValueEnum<M> {
    /// Relative to this value, which must be a Struct or Embed for the first segment to apply.
    pub fn resolve(&self, path: &PropertyPath) -> Result<&PropertyValueEnum<M>, ResolveError>;
    pub fn resolve_mut(&mut self, path: &PropertyPath) -> Result<ValueSlot<'_, M>, ResolveError>;
}
impl<M> Bin<M> {
    pub fn resolve(&self, object_hash: impl Into<BinHash>, path: &PropertyPath)
        -> Result<&PropertyValueEnum<M>, ResolveError>;
    pub fn resolve_mut(&mut self, object_hash: impl Into<BinHash>, path: &PropertyPath)
        -> Result<ValueSlot<'_, M>, ResolveError>;
    pub fn patch(&mut self, object_hash: impl Into<BinHash>, path: &PropertyPath, value: PropertyValueEnum<M>)
        -> Result<Option<PropertyValueEnum<M>>, PatchError>;
}
```

Everything is inherent (M-ESSENTIAL-FN-INHERENT), receivers are plain references
(M-AVOID-WRAPPERS), and `M` stays free so `ltk_ritobin` can resolve over `PropertyValueEnum<Span>`.

`resolve_mut` returns a `ValueSlot` ([section 5.5](#s5.5)) rather than a `&mut PropertyValueEnum`,
because replacing a value inside a container can change its kind and the slot is what refuses a
replacement its holder does not allow. Beyond that it performs no type checking: `patch` is the
client-faithful operation.

### <a id="s9.2"></a>9.2 Traversal rules

| value at the cursor                                                | next piece     | result                                                                                                                   |
| ------------------------------------------------------------------ | -------------- | ------------------------------------------------------------------------------------------------------------------------ |
| object, Struct with class != 0, Embed                              | `.name`        | property by `name_hash`; absent: `MissingProperty` (`patch` inserts when it is the last segment)                         |
| Struct with class 0                                                | anything       | `NullPointer`                                                                                                            |
| List, List2                                                        | `[i]`          | element `i`; `i >= len`: `IndexOutOfRange`                                                                               |
| Option                                                             | `[0]`          | the contained value; absent or `i > 0`: `IndexOutOfRange`                                                                |
| Map                                                                | `{k}`          | the entry whose key equals `k` converted to the key kind; literal not convertible: `InvalidKey`; no entry: `KeyNotFound` |
| List, List2, Option, Map                                           | `.name`        | `CannotDescend`                                                                                                          |
| object, Struct, Embed                                              | `[i]` or `{k}` | `NotIndexable`                                                                                                           |
| any leaf kind (primitives, String, Hash, WadChunkLink, Link, Flag) | anything       | `CannotDescend` or `NotIndexable`                                                                                        |
| element reached through a subscript                                | more segments  | continue only if it is a Struct or Embed, else `CannotDescend`                                                           |

Link (0x84) is a leaf. The client does not follow links while resolving a path; only Pointers are
dereferenced.

### <a id="s9.3"></a>9.3 The type rule in `patch`

The client compares the record's tag with the resolved property's registered tag, and for a
container the element tags too. For an **Embed** it compares `MetaClass` pointers, so the class must
match exactly. For a **Pointer** it walks the primary base chain and then the secondary-base pairs:
a class deriving from the declared one is accepted and constructed as the file's class, and an
unrelated or unresolvable class is skipped. That is an is-a test, and running it needs a class
hierarchy only the game has.

Without a schema the closest faithful check is against the value currently in the tree, with a
pointer's class left out entirely - which accepts strictly more than the client does rather than
guessing (ADR-0003):

```rust
/// What the type rule compares: kind, container item kinds, and the class of an Embed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueShape {
    pub kind: Kind,
    pub item_kind: Option<Kind>,   // List, List2, Option: the item kind; Map: the value kind
    pub key_kind: Option<Kind>,    // Map only
    pub class: Option<BinHash>,    // Embed only; Pointer is polymorphic and is not compared
}
impl ValueShape {
    pub fn of<M>(value: &PropertyValueEnum<M>) -> Self;
    pub fn matches(&self, other: &Self) -> bool;
}
// Display uses this crate's own `Kind` names, not ritobin's (D16):
// "Vector2", "Container[I32]", "Map[Hash, String]", "Embedded 4eb9ba4f"
```

- Leaf present: `ValueShape::of(existing).matches(&ValueShape::of(&value))`, otherwise
  `PatchError::TypeMismatch { expected, found }` and nothing changes.
- Leaf absent and the parent is an object, Struct or Embed: insert, with no check possible. 2,464
  shipped records rely on it ([appendix B](#appendix-b)). **The client has no counterpart to an
  insert at all**: it patches a live object that was in-place constructed from its class before
  deserialization, so every property the class declares already exists at its offset holding
  whatever the constructor left there, and the file only overwrites the ones it carries. There is
  no absent leaf in the client and no insert. Both halves of D8 are toolkit decisions with nothing
  to check them against, and the skip half does strictly less than the game does.
- A Pointer value replaces the existing one wholesale whatever its class, which is what the client
  does for an accepted descendant.

### <a id="s9.4"></a>9.4 Errors

```rust
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{kind} (segment {segment})")]
pub struct ResolveError { segment: usize, kind: ResolveErrorKind }
impl ResolveError {
    /// Index of the segment that failed; 0 for `MissingObject`.
    pub fn segment(&self) -> usize;
    pub fn kind(&self) -> ResolveErrorKind;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResolveErrorKind {
    MissingObject(BinHash),    // Bin-level only
    MissingProperty(BinHash),
    NullPointer,
    CannotDescend(Kind),
    NotIndexable(Kind),
    IndexOutOfRange { index: u32, len: usize },
    InvalidKey(Kind),
    KeyNotFound,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PatchError {
    #[error(transparent)]
    Resolve(#[from] ResolveError),
    #[error("type mismatch: the property is {expected}, the patch carries {found}")]
    TypeMismatch { expected: ValueShape, found: ValueShape },
}
```

### <a id="s9.5"></a>9.5 `BinOverride::apply` and `check`

Order, mirroring `ReadPropertyHeader_unk`:

1. Remove every hash in `deleted` from `base.objects`.
2. Insert the patch's objects, skipping those whose hash is in `deleted`. An object whose hash is
   already in the base replaces it (D7).
3. Apply the records in file order against the merged table through `Bin::patch`. A failure is
   recorded and the loop continues; nothing is fatal, because that is the client's behaviour.

`apply` takes `self`: the patch's objects and record values move into the base and nothing is
cloned, so there is no `M: Clone` bound. Applying one patch to several bases is
`patch_bin.clone().apply(...)`, which makes the copy the caller's explicit choice (C-CALLER-CONTROL,
D17).

```rust
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ApplyReport {
    pub deleted: Vec<BinHash>,     // objects actually removed
    pub added: Vec<BinHash>,       // patch objects inserted
    pub replaced: Vec<BinHash>,    // patch objects that replaced a base object
    pub applied: usize,            // records applied (for `check`: applicable)
    pub inserted: usize,           // of those, records that created the leaf
    pub skipped: Vec<SkippedPatch>,
}
#[derive(Debug, Clone, PartialEq)]
/// Self-contained: the path is kept because `apply` has consumed the patch by the time the report is read.
pub struct SkippedPatch { pub index: usize, pub object_hash: BinHash, pub path: PropertyPath, pub error: PatchError }
impl ApplyReport {
    pub fn is_clean(&self) -> bool;   // no skips
}
// Display: "109 applied (3 inserted), 0 skipped, 0 deleted, 0 added, 0 replaced"
```

`check` runs the same walk without mutating and returns the same report. It answers "does this
patch still apply to this version of the base", which is the question a mod manager asks after a
game update.

It differs from `apply` in one way worth stating: **`check` judges every record against the base as
it stands, where `apply` runs them in file order.** A record that only fits because an earlier
record in the same patch replaced a pointer or an embed above it is therefore judged against the
value that earlier record would have overwritten.

**A wildcard patch has no distinct outcome.** A patch whose target hash is 0 is offered to every bin
the client parses, and applies only the records whose object hashes that bin holds (PRD-001
[section 5](../prd/001-ptch-property-patches.md#s5)). `apply` and `check` charge a record whose
object is absent to `ApplyReport::skipped`, which is right for a patch aimed at one target and wrong
for a wildcard offer: a correct wildcard would report almost entirely skips and `is_clean()` would
be false everywhere. If declarative targeting grows wildcard support, "this bin is not this record's
target" has to become an outcome distinct from "this record is broken". Not yet ticketed.

## <a id="s10"></a>10. Merge

`Bin::merge` layers one bin over another in place: `edited` wins at every leaf it reaches, and
anything only `base` has survives. It is what `ltk-manager`'s overlay build runs (its ADR-0012), and
the operation FR-7 asks for.

**Merge is not a diff followed by an apply** (ADR-0004). The tempting shape is one operation - diff
the two bins into a `BinOverride`, then `apply` it, so both input paths converge on the record
language. It does not hold, for one reason:

**A record cannot insert a map entry.** `patch_in` creates a leaf only when the last segment names
it outright and its parent is an object, a Struct or an Embed ([section 9.3](#s9.3), and the 2,464
shipped records that rely on it). A `{key}` subscript needs an entry to subscript, so `KeyNotFound`
is the only answer for a key the base does not have. A mod that adds 84 map keys therefore has no
record set that says so; the closest expressible record carries the whole map, which is exactly the
wholesale replacement ADR-0012 exists to stop.

So the merge is an operation in its own right, and the diff is a second one that renders as much
of the same walk as records can carry:

- `Bin::merge` layers one bin over another in place. No serialization constraint, so it can insert
  map keys, and it needs no field names.
- `Bin::diff` produces a `BinOverride` for export, escalating where a record cannot say what the
  walk found, and reporting every escalation.

They share one descent. The invariant that ties them is in [section 12](#s12).

### <a id="s10.1"></a>10.1 The merge walk

`edited` wins at every leaf it reaches; anything only `base` has survives. Absence in `edited` is
never a difference - that is the whole of ADR-0012, and the record language has no way to express a
removal in any case.

| base                                 | edited                | action                                                                                            |
| ------------------------------------ | --------------------- | ------------------------------------------------------------------------------------------------- |
| property absent                      | any                   | insert `edited`'s value                                                                           |
| Struct or Embed, same class          | same kind, same class | recurse field by field                                                                            |
| Struct or Embed, different class     | any                   | replace                                                                                           |
| Struct with class 0 (a null pointer) | any                   | replace                                                                                           |
| Map, same key and value kinds        | Map                   | recurse on common keys, append `edited`'s new ones in its order, keep base-only keys              |
| Map, different key or value kinds    | any                   | replace                                                                                           |
| Container, UnorderedContainer        | any                   | replace whole. A list has no key to combine by, and ADR-0012's "a plain value replaces" covers it |
| Optional, both present               | Optional              | recurse into the contained value                                                                  |
| Optional, either absent              | Optional              | replace                                                                                           |
| any leaf kind                        | equal value           | nothing                                                                                           |
| any leaf kind                        | different value       | replace                                                                                           |
| any                                  | different kind        | replace, and count it in the report                                                               |

Key equality is `key_eq` from [section 9](#s9), so metadata is ignored there. A `Map` merge is
quadratic on entry counts unless the walk indexes one side first; shipped maps reach the low
thousands of entries, so index the base side.

Dependencies merge as a union: `base`'s list in its order, then anything only `edited` has.

```rust
/// What a merge did: what it overwrote, and what it added.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct MergeReport<M = NoMeta> {
    /// Objects taken whole from the edit because the base had no such hash.
    pub objects_added: Vec<BinHash>,
    /// Objects that existed on both sides and were combined.
    pub objects_merged: Vec<BinHash>,
    /// Every leaf the edit overwrote, with the value the base held there.
    pub replaced: Vec<Replaced<M>>,
    /// Properties the base object did not have.
    pub inserted: usize,
    /// Map entries the base map did not have.
    pub keys_inserted: usize,
}

/// One leaf the edit overwrote.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Replaced<M = NoMeta> {
    /// Where it happened.
    pub at: ValuePath,
    /// What the base held. Moved out of the base rather than cloned, so recording every
    /// replacement costs nothing a merge was not already paying.
    pub was: PropertyValueEnum<M>,
    /// Whether the two sides held different kinds, or a Struct of a different class.
    ///
    /// This is the one a user needs shown. It is not a curiosity: an exact-tag mismatch
    /// between a mod's value and the game's is the signature of a **type migration**, and
    /// Riot performs those in place - 337 times in three years, then 327 in the single
    /// 16.17 `String` -> `File` patch, hitting `StaticMaterialShaderSamplerDef.texturePath`
    /// and `AnimationResourceData.mAnimationFilePath`, which is to say retexturing and
    /// custom animations. The client applies the tag rule exactly and drops a value whose
    /// tag does not match, silently, so a mod that predates the migration loses those
    /// fields with nothing said. Merging writes the mod's stale value through, which
    /// reproduces the loss; this flag is what lets a caller catch it first.
    pub mismatched: bool,
}

impl<M: Clone + PartialEq> Bin<M> {
    /// Layers `edited` over this bin, in place: ADR-0012's merge.
    pub fn merge(&mut self, edited: &Self) -> MergeReport<M>;
}
impl<M: Clone + PartialEq> BinObject<M>         { /* the same, over properties */ }
impl<M: Clone + PartialEq> values::Struct<M>    { /* the same */ }
impl<M: Clone + PartialEq> PropertyValueEnum<M> { /* the same, one value against one value */ }
```

`M: PartialEq` is what decides "different value", so a metadata that varies per occurrence makes
every leaf differ. `ltk_ritobin`'s `PropertyValueEnum<Span>` is that case: map it through
`no_meta()` before merging, or merge the `NoMeta` trees and re-print. Stated, not solved.

### <a id="s10.2"></a>10.2 What a stale mod looks like in the report

PRD-001 [section 6](../prd/001-ptch-property-patches.md#s6) ranks the failure modes a mod meets on a
build its author never saw, measures the one that actually happens, and states the guarantee a merge
carries. What that leaves for this document is which part of the report answers each:

- **A type migration** is caught here with nothing captured and no schema, because the mod's stale
  value and the game's current one differ in kind: `Replaced::mismatched` marks every one.
- **A moved or renamed property** is what `check` already reports ([section 9.5](#s9.5)).
- **A changed base value underneath the mod** is not chased, and there is no `Baseline` type: a mod
  is authoritative where it speaks (ADR-0006).

## <a id="s11"></a>11. `ValuePath`: where the walk is, without needing a name

A `PropertyPath` is text, and `Segment::name_hash` is FNV-1a of that text ([section 8](#s8)), so
writing one needs the property's plaintext name. A bin stores name hashes only. A walk over two bins
therefore cannot always spell where it is: an unknown hash has no path.

Rather than grow a hash escape into the client's grammar - `0x1234abcd` and `#1234abcd` are both
legal `PropertyPath` names today and would hash as their own text, so any escape both breaks
`PropertyPath::new` and produces text the client misreads - the walk gets its own address type
that never claims to be a client path:

```rust
/// Where a merge or diff touched, addressed by hash. Total: every position in a value tree has
/// one. Not a client path - it may name a field whose plaintext is unknown, and it is never
/// written to a file.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ValuePath(Vec<Step>);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Step {
    /// A property or field, by name hash.
    Field(BinHash),
    /// A container element, or the value inside an Option.
    Index(u32),
    /// A map entry, by its key value.
    Key(PropertyValueEnum),
}

impl ValuePath {
    /// The client path naming the same position, if every field hash has a known name.
    ///
    /// # Errors
    ///
    /// [`Unnameable`] naming the first field hash `names` could not spell, or the first `Key`
    /// step whose key kind has no `{...}` literal.
    pub fn to_property_path(&self, names: &dyn FieldNames) -> Result<PropertyPath, Unnameable>;
}

/// Plaintext for bin field hashes. `ltk_ritobin::hashes` already has an implementation of this
/// shape; this is the smaller trait `ltk_meta` can own without depending on it.
pub trait FieldNames {
    fn field(&self, hash: BinHash) -> Option<&str>;
}
```

The argument for the type is **totality**, not the avoidance of a name table. Every position in a
value tree has a `ValuePath`, including positions that have no name and never will: an element of a
container, an entry of a map. A report addressed in `PropertyPath` would have to give up on exactly
the positions a user most needs told about. `PropertyPath` is the export language; `ValuePath` is
the reporting language.

An earlier draft of this section justified it as sparing the overlay build a hashtable. That was
wrong twice over: `lol-meta-classes` resolves field names, and `ltk-manager` ADR-0009 already gates
its check, sweep and repair on `ModLibrary::hashtables_ready()`, so a hashtable is present wherever
this code runs.

## <a id="s12"></a>12. The diff

**Parked.** Designed here, not scheduled. Its only consumer would be an authoring flow that turns
a modder's edited bin into a `PTCH` on the install it was made on, and no such flow exists yet;
the manager's overlay build needs `merge`, not `diff`. Same treatment as the streaming design's
follow-on resolver: written down so the shape is settled, built when something asks for it.

```rust
/// Whether a diff may say that the edit dropped an object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct DiffOptions {
    /// An object in the base and not in the edit goes on [`BinOverride::deleted`].
    ///
    /// Off by default: a mod that omits an object is not asking for it to be deleted (ADR-0012).
    /// A tool authoring a deliberate patch turns it on.
    pub deletions: bool,
}

/// Where the record language could not say what the walk found.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Lift {
    /// The edit adds a map entry. No record inserts one, so the whole map went into one record,
    /// and base-only keys of any other base will not survive it.
    MapInsert { at: ValuePath, keys: usize },
    /// A field hash on the path has no known name, so the record was written at the nearest
    /// nameable ancestor, or the whole object was taken.
    Unnameable { at: ValuePath, hash: BinHash },
    /// The two sides held different kinds or classes here, so the ancestor replaced whole.
    Mismatch { at: ValuePath },
}

#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct DiffReport {
    /// Records emitted.
    pub records: usize,
    /// Objects taken whole into [`BinOverride::objects`].
    pub objects: Vec<BinHash>,
    /// Objects put on the delete list. Always empty unless [`DiffOptions::deletions`].
    pub deleted: Vec<BinHash>,
    /// Every place the walk stopped short of the leaf, in walk order.
    pub lifted: Vec<Lift>,
}

impl<M: Clone + PartialEq> Bin<M> {
    /// The patch that turns this bin into `edited`, as far as records can say it.
    pub fn diff(&self, edited: &Self, names: &dyn FieldNames) -> (BinOverride<M>, DiffReport);
    /// See [`DiffOptions`].
    pub fn diff_with(&self, edited: &Self, names: &dyn FieldNames, options: &DiffOptions)
        -> (BinOverride<M>, DiffReport);
}
```

The escalation ladder, applied at the first position the record language cannot carry: a record at
the leaf, else a record carrying the whole value at the nearest expressible ancestor, else the whole
object into `BinOverride::objects`. Each rung taken lands in `DiffReport::lifted` with its reason,
so an authoring tool can tell an author what their patch will not survive.

**The invariant.** For any `base` and `edited`,

```text
base.diff(edited).apply(base)  ==  base.merge(edited)      when DiffReport::lifted is empty
```

and where it is not empty, every position the two results differ at is named by a `Lift`. That is
the property test, and it is the honest statement of what an exported `.ptch` costs against the
in-process merge. With `deletions` on and nothing lifted, the result is `edited` itself, except for
properties and keys `edited` dropped inside an object it kept - those survive, by design and
because no record can remove them.

## <a id="s13"></a>13. Joining several patches

Records are an ordered list, so overrides concatenate and apply in order, last writer winning. What
a build wants first is to know when that matters:

```rust
/// Two overrides that write the same place.
///
/// `overrides` is always a pair of positions in the order `join` was given, earlier first.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Collision {
    /// Two records address the same object and path.
    Record { object_hash: BinHash, path: PropertyPath, overrides: (usize, usize) },
    /// One override replaces an object whole; another patches inside it.
    Object { object_hash: BinHash, overrides: (usize, usize) },
    /// One override deletes an object another writes to.
    Deleted { object_hash: BinHash, overrides: (usize, usize) },
}

/// Concatenates the overrides in order and reports every place two of them meet.
pub fn join<M>(overrides: impl IntoIterator<Item = BinOverride<M>>)
    -> (BinOverride<M>, Vec<Collision>);
```

`join` reports and does not resolve: which override should win is the caller's policy, and a
manager that knows the user's load order has more to go on than this crate does.

## <a id="s14"></a>14. The per-record surface a stripper needs

The boundary is **ADR-0006**: reproducing the client's apply is `ltk_meta`'s work, judging a mod
against Riot's meta classes is not. Stripping records that say nothing therefore runs outside the
crate, as a post-pass over a finished `BinOverride`, and what the crate owes that post-pass is a
per-record answer - `check` reports only aggregate counts today.

```rust
/// What one record did, or would do. Reported per record, in file order, so a caller can
/// decide what to keep without walking the base itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordOutcome {
    /// The leaf existed and the value replaced it.
    Replaced,
    /// The leaf did not exist and was created. The case a schema-holding caller can strip.
    Inserted,
    /// The record did not apply. `ApplyReport::skipped` says why.
    Skipped,
}

impl ApplyReport {
    /// Per record, in file order.
    pub fn outcomes(&self) -> &[RecordOutcome];
}

impl<M> BinOverride<M> {
    /// Drops records `keep` rejects, judging each against `base` the way `check` does.
    ///
    /// The predicate sees the record and what applying it would do, which is everything a
    /// caller needs to consult a schema and decide.
    pub fn retain_with(&mut self, base: &Bin<M>,
        keep: impl FnMut(&PropertyPatch<M>, RecordOutcome) -> bool);
}
```

Four things this surface has to get right:

- **The predicate, not the crate, holds the rule.** `retain_with` judges each record against the
  base exactly as `check` does and asks; it never consults a default itself.
- **Only one of the two no-op cases needs a schema, and it is the insert.** A record whose leaf the
  base does not serialize says nothing exactly when its value equals the meta class default, which
  takes a dump to know. The other case - a record whose value equals what the base already
  serializes - is never emitted by a correct diff. And a record setting the meta class default over
  a base that serializes something else is **not** a no-op: stripping it silently reverts the mod.
  Whoever writes the post-pass needs all three of those sentences.
- **D8 and D11 stay as taken.** With the base chain in hand a Pointer's class could be compared
  exactly, and an absent intermediate Embed could be default-constructed the way the client does,
  closing the 69 shipped records that skip on one. Both are declined, because both would put a
  schema inside the walk.
- **A merge must never refuse.** Where a stripper can decline to run because the dump does not
  cover a build, the merge cannot: refusing leaves the user with the crash the merge exists to
  fix. The recommendation to a manager is to merge in degraded mode and record which mode ran.

## <a id="s15"></a>15. ritobin text

moonshadow's ritobin already has a text shape for PTCH. Its binary reader emits a `patches` section
of type `map[hash,embed]` whose values are `patch` embeds with two fields, `path: string` and
`value`, and it records the inner PROP version as `version`
(`ritobin_lib/src/ritobin/bin_io_binary_read.cpp`, `_write.cpp`). LtMAO's `pyRitoFile` uses the
same vocabulary (`is_patch`, `patches`, fields `hash`, `path`, `type`, `data`). Matching it means
existing `.py` / `.rito` files keep working and our output diffs cleanly against theirs:

```text
#PROP_text
type: string = "PTCH"
version: u32 = 3
linked: list[string] = {}
entries: map[hash,embed] = {}
patches: map[hash,embed] = {
    "ClientStates/Gameplay/UX/LoL/LoLMinimap/UIBase/Minimap/VoiceChatButton/VoiceChatPanel_ButtonClicked" = patch {
        path: string = "Position.Anchors.Anchor"
        value: vec2 = { 0, 1 }
    }
    0x4a47c414 = patch {
        path: string = "Position.UIRect"
        value: embed = UiElementRect {
            Position: vec2 = { 0, 0 }
            Size: vec2 = { 310, 311 }
            SourceResolutionWidth: u16 = 1920
            SourceResolutionHeight: u16 = 1080
        }
    }
}
```

Rules:

- `patches` keys repeat (one object, many records). `values::Map` is a `Vec` of pairs, so the
  typechecker keeps duplicates and order; it must not collapse them into an `IndexMap`.
- `deleted: list[hash] = { }` is a new root entry for the delete list. It is ours only; it is
  omitted when empty so every shipped file prints exactly as ritobin prints it (D14).
- On a `PTCH` file, `linked` must be empty (diagnostic, D3) and `version` must be 3. `entries` and
  `linked` are still printed when empty, as ritobin does.
- `RootKind` gains `Patches` and `Deleted`. `Cst::build_bin` is joined by `Cst::build` returning
  `(BinFile, Vec<DiagnosticWithSpan>)`; `build_bin` keeps its signature and diagnoses a `PTCH`
  file. `Print` is implemented for `BinOverride` and `BinFile`.
- `path` is a plain `string`. There is no new literal syntax, and the path is not unhashed or
  rehashed: property names inside it are already names.

On #173's framing: the language is Riot's own `PropertyPathIterator` grammar (dotted members,
`[i]`, `{k}`), not rapidjson's JSON Pointer (`/a/b/0`, RFC 6901). rapidjson is involved only in
parsing the `{k}` key text (`PropertyPath.hpp`).

## <a id="s16"></a>16. Testing

Fixtures, added under `crates/ltk_meta/tests/bins/` next to `leona_small.bin`:

- `lolminimap_uiflipped.ptch.bin`, 5762 bytes, 0 objects, 109 records (the flipped minimap).
- `lolminimap_uibase.bin`, 18,256 bytes, 66 objects (its base).

Both come from `UI.wad.client` of client 16.16.804.9184.

**Reading, writing and the path language:**

- Tokenizer table: every row of [section 8.1](#s8.1), plus `push_field` / `push_index` / `push_key`,
  `Display` and `FromStr` round trips, the serde string form.
- Round trips through `to_writer` then `from_reader` for a builder-made patch covering every value
  kind, mirroring `tree/tests.rs`.
- The fixture reads, re-writes byte-identical, and its parsed form is an insta `.ron` snapshot.
- Error paths: `PROP` magic into `BinOverride::from_reader`, `PTCH` magic into `Bin::from_reader`,
  outer version 2, inner version 2, one dependency, a truncated record, a bad path.

**Resolution and apply:**

- A synthetic tree that exercises every row of [section 9.2](#s9.2) and every `ResolveErrorKind`.
- `check(uiflipped, uibase)` as a snapshot: 109 applicable, the insert count, 0 skipped.
- `apply` then `resolve` spot checks: `VoiceChatPanel_ButtonClicked` anchor goes from `(1, 1)` to
  `(0, 1)`, `MinimapFrame.FlipX` appears as `true`.
- Corpus test, `#[ignore]`, enabled by `LTK_LOL_GAME_DIR`: every PTCH chunk in the install reads,
  re-writes byte-identical, and `check` against its `uibase` reproduces the numbers in
  [section 3.1](#s3.1).

**ritobin text:** print and parse snapshots for the fixture, and diagnostics for a non-empty
`linked` on a `PTCH`, a `patches` entry that is not a `patch`, and a `path` that fails
`PropertyPath::new`.

**Merge, diff and join:**

- **Property tests.** `merge` is idempotent (`base.merge(e).merge(e)` equals `base.merge(e)`) and
  absorbing (`base.merge(base)` equals `base`).
- **Corpus, against an install.** Every shipped `PTCH` applied to the objects it names, with
  `ApplyReport::outcomes` asserted against the counts in [appendix B](#appendix-b), so the
  per-record surface cannot drift from the aggregate one.
- **Parked with the diff**, for whenever it is built: diff each applied result back against the
  objects it came from and assert the record set matches the original modulo `Lift`s, plus
  [section 12](#s12)'s invariant on generated pairs. 23,047 shipped records is a larger diff corpus
  than anything written by hand.
- **The ADR's specimen.** The 847-object mod bin over the 1,473-object game bin: merged, all 1,151
  dropped `ResourceResolver` keys are present, all 4,788 of the mod's own bindings survive, and the
  84 it adds are there. That fixture is the manager's; `ltk_meta` gets the reduced case.
- **`ValuePath` round trip.** `to_property_path` then `resolve` lands on the value the walk was at,
  for every position in a fixture tree with a complete name table.

## <a id="s17"></a>17. Rules

Every rule too small to hold a section of its own, in one table, ordered by subject. **Rule** is
what the crate does, **Instead of** the alternative weighed and rejected, **Spec** where the
behaviour is specified in full - so nothing is restated here. A row whose Spec names an **ADR** is
argued there, with the options it beat and what it costs; the row states the rule and no more.

`Dn` is a stable citation key. A rule that changes keeps its ID and has its row rewritten; new
rules append.

| ID | Rule | Instead of | Why | Spec |
| -- | ---- | ---------- | --- | ---- |
| D1 | The file is `BinOverride`, its records are `PropertyPatch`, the per-property verb is `patch()`, the private module is `data_override`. | `BinPatch`; a module or method named `override`. | `Override` is the game's own noun (`BinFileCache_addDataOverride`, `PropertyOverrideLoadable`, `cache->overrides`) and `ltk_file`'s (`LeagueFileKind::PropertyBinOverride`); patch is the record's noun in `PTCH`, LtMAO and ritobin; `override` is reserved in Rust. | [section 2](#s2), [section 5.1](#s5.1), [section 5.2](#s5.2); ADR-0001 |
| D29 | A `BinOverride` is never called a layer. `layer` stays in use as a verb. | A third noun for the same file. | The reversing notes' "layer" means the client's cache entry attaching a patch to a base bin, which is a different thing from this crate's type. `override` alone is reserved in Rust; the plural is not, so `join` reports `overrides: (usize, usize)`. | [section 2](#s2); ADR-0001 |
| D28 | **class** is the hash a bin object carries; a **meta class** is one class's dumped definition; a **schema** is the collection for a build. | Using "schema" for all three. | "Meta class" is Riot's own word and the dump repository's, and this document uses it for nothing else. | [section 2](#s2); ADR-0006 |
| D16 | `ValueShape` keeps its name and this crate's kind vocabulary. | Naming it, or printing it, in ritobin's type words. | `ltk_ritobin::RitoType` can be built on it once both exist; two names per kind inside `ltk_meta` would not pay. | [section 9.3](#s9.3); ADR-0003 |
| D2 | No version field is exposed or settable. `to_writer` emits `PTCH` 1 and `PROP` 3 as constants. `Bin` keeps `version`. | Caller-settable versions on `BinOverride`. | The client accepts exactly one value for each. `PROP` v2 files exist in the wild, so `Bin` still needs the field. The read side is D15. | [section 4](#s4), [section 7](#s7); ADR-0002 |
| D15 | `Bin::from_reader` accepts `PROP` 1 to 3, and `BinOverride::from_reader` inner 1 to 3 with the record list read only for 3. Both writers emit 3. | Enforcing the client's version gates on read. | The toolkit has always read every version it knows; the client's gates are documented in [section 4](#s4) instead of enforced. D3 is unaffected, because dependencies are not a version. | [section 4](#s4), [section 7](#s7); ADR-0002 |
| D3 | A non-zero `dependencyCount` is `OverrideDependencies`, and dependencies are not representable. | Reading and keeping them, erroring on write. | A patch that declares any cannot load in the client. | [section 7](#s7); ADR-0002 |
| D5 | A `payloadSize` that disagrees with the body is `InvalidSize`. | Trusting the field, as the client does. | Every other size field in the crate is enforced. | [section 7](#s7); ADR-0002 |
| D6 | Object kinds get the legacy-numbering retry, which rewinds to the start of the table. Record kinds are read as non-legacy, with no retry. | Retrying record kinds too. | Records exist only in inner version 3, which postdates the renumbering, so a legacy record list cannot exist; one that somehow did would fail `InvalidPropertyTypePrimitive` rather than be misread. | [section 7](#s7); ADR-0002 |
| D4 | Record paths are parsed and validated on read; a malformed one is `InvalidPropertyPath`. | A raw `String` in the record, validated at resolve time. | The typed form is the point of the path work, and every shipped record parses. | [section 7](#s7), [section 8](#s8) |
| D12 | `BinFile` and `BinKind` are part of the reading surface. | Leaving callers to sniff the magic themselves. | Small, and the entry point every scanning tool wants. | [section 5.4](#s5.4) |
| D13 | A `PropertyPath` addresses a property inside one object; the object hash is a separate argument (`Bin::resolve(object_hash, path)`). | One string addressing a whole file. | Whole-file addressing can extend the grammar later without touching the object-level API. | [section 5.3](#s5.3), [section 9.1](#s9.1) |
| D9 | `Option[0]` resolves; no other index on an Option does. | Rejecting subscripts on Option outright. | The 16.14 decompile of `MetaPath_resolve`. Untested in game. | [section 8.1](#s8.1) |
| D10 | `{key}` text is parsed as JSON and coerced to the map's key type, per `PropertyPath.hpp`. An enum-typed key is not supported. | Taking the brace text as a literal string, which the other client source implies. | The two sources disagree and no shipped record uses `{key}`, so data cannot settle it. Enum keys need a schema. | [section 8.1](#s8.1) |
| D19 | `ValuePath` is a separate type from `PropertyPath`, addressing by hash and by position. | A hash escape in the path grammar. | `ValuePath` is total - container elements and map entries have no name - and `PropertyPath` keeps its promise that every one of them is client-resolvable. | [section 11](#s11); ADR-0005 |
| D8 | A missing intermediate segment is a skip (`MissingProperty`). A missing leaf whose parent exists is created. | Default-constructing the absent intermediate from its class. | The client has no counterpart to either half: it patches an object whose every declared property already exists, so the skip does strictly less than the game. A schema hook can arrive later as `apply_with(&self, base, &ApplyOptions)` without a break. | [section 9.2](#s9.2), [section 9.3](#s9.3); ADR-0006 |
| D11 | The type rule compares `ValueShape`: kind, item and key kinds, Embed class exact, Pointer class ignored. | Comparing a Pointer's class. | The client runs an is-a test up the base chain, which needs a hierarchy only the game has; omitting the class accepts strictly more than the client rather than guessing. | [section 9.3](#s9.3); ADR-0003 |
| D7 | A patch object whose hash is already in the base replaces it, reported in `ApplyReport::replaced`. | Keeping both, as the client does. | The client's merged table would hold both with unspecified lookup order. Unattested in shipped data. | [section 9.5](#s9.5) |
| D17 | `apply(self, base)` consumes the patch. `check(&self, base)` borrows. | `apply(&self, base)`, cloning internally. | Moving clones nothing and needs no `M: Clone`; reuse is the caller's explicit `clone()` (C-CALLER-CONTROL). | [section 9.5](#s9.5) |
| D22 | Containers replace whole. No element-wise merge and no LCS. | A positional or keyed element merge. | ADR-0012's semantics are the client's, and a list has no key to combine by. | [section 10.1](#s10.1); ADR-0004 |
| D24 | `merge` and `diff` need `M: PartialEq` and compare whatever `M` compares. | Comparing values while ignoring metadata. | A span-carrying tree goes through `no_meta()` first; a crate-level exception would surprise everyone who did not want it. | [section 10.1](#s10.1) |
| D21 | `DiffOptions::deletions` is off by default: a bin that omits an object says nothing about it. | Emitting a delete for every absent object. | Omission is how mods are authored; deliberate deletion is a tool's explicit choice. | [section 12](#s12) |
| D27 | `Bin::diff` is designed and parked, not scheduled. | Building it alongside `merge`. | Its only consumer would be an authoring flow that does not exist; the overlay build needs `merge`. | [section 12](#s12); ADR-0004 |
| D23 | `join` reports collisions; `apply` resolves them by applying in order, last writer winning. | `join` picking a winner. | Which override should win is policy, and a manager that knows the user's load order has more to go on than this crate. | [section 13](#s13) |
| D25 | No schema enters `ltk_meta`. Reproducing the client's apply is in scope; judging a mod against Riot's meta classes is not. | A schema trait taken by `apply_with` / `strip_noops`. | The crate must work with no dump present, and a dump is build-versioned data. | [section 14](#s14); ADR-0006 |
| D26 | Stripping no-op records is a post-pass outside the crate, served by `ApplyReport::outcomes` and `BinOverride::retain_with`. | A `strip_noops` inside `ltk_meta`. | Follows from D25. Only the insert case needs a default at all, and stripping the wrong one silently reverts the mod. | [section 14](#s14); ADR-0006 |
| D20 | There is no `Baseline`. Nothing is captured at authoring time. | Capturing every record's authored-over value to detect drift under it later. | A mod is authoritative where it speaks, so the report would fire on every record after every patch; the failure that hurts is caught from a dump with nothing captured. | [section 10.2](#s10.2); ADR-0006 |
| D14 | The ritobin root for the delete list is `deleted: list[hash]`, omitted when empty. | Always emitting it. | Every shipped file then prints exactly as ritobin prints it. | [section 15](#s15) |
| D18 | The fixtures are real game files, committed to the repo. | Synthesised fixtures. | Both are small enough to sit next to `leona_small.bin`. | [section 16](#s16) |
| D30 | `ltk_meta::Error` stays one public `thiserror` enum; `PropertyPathError` and `ResolveError` are structs carrying a position and a public kind. | Situation-specific error structs with a private kind, per M-ERRORS-CANONICAL-STRUCTS. | The crate convention is the enum, and C-GOOD-ERR is met either way. The kind stays public rather than hiding behind `is_xxx()` helpers, because callers classify skips out of `ApplyReport`. | [section 6](#s6), [section 8.3](#s8.3), [section 9.4](#s9.4) |
| D31 | The new builder is `BinOverrideBuilder`. | `Builder`, matching what `Bin` exposes. | M-INIT-BUILDER wants `FooBuilder`; renaming `Bin`'s `Builder` to `BinBuilder` is a separate cleanup, not this one's to make. | [section 5.1](#s5.1) |
| D32 | `BinOverride` and `PropertyPatch` expose their fields; `PropertyPath` keeps its private. | C-STRUCT-PRIVATE throughout. | `Bin` and `BinObject` are public-field types and the new ones sit beside them. `PropertyPath` is the exception because it carries a validated invariant. | [section 5.1](#s5.1), [section 5.2](#s5.2), [section 5.3](#s5.3) |

## <a id="appendix-a"></a>Appendix A. Client functions

16.14-era exe, image base `0x140000000`:

| address       | name                                                    |
| ------------- | ------------------------------------------------------- |
| `0x1411B3BC0` | `ReadPropertyHeader_unk`, base and patches in lock-step |
| `0x1411B3450` | `PropertyPatch_readAndApply`, one record                |
| `0x1411B7730` | `MetaPath_resolve`, the path language                   |
| `0x1411B4DD0` | `MetaValue_readInto`                                    |
| `0x1411B87B0` | `MetaValue_skipByType`                                  |
| `0x1411A95E0` | `MetaClass_adjustPointerTo`                             |

16.16.8042073: `MetaFile_readImpl` at `0x1411B6D90` (the version gates and the delete list),
`MetaFile_readEntry` at `0x1411B7940` (the delete set applied per entry).

## <a id="appendix-b"></a>Appendix B. Corpus measurements

Client **16.16.804.9184**, every `.wad.client` in the install, 456 archives. Run by
`crates/ltk_meta/tests/corpus.rs`, which is ignored unless `LTK_LOL_GAME_DIR` points at an install.

The test collects every object a patch's records name, by hash, from the archive the patch lives
in: a bin object's path hash is the hash of its asset path, so a record lands on the object it
names without anything needing to know which file that is.

| measurement                                                   | result           |
| ------------------------------------------------------------- | ---------------- |
| PTCH chunks read, re-written and compared byte for byte       | 238 of 238       |
| `BinFile::from_reader` agrees with `BinOverride::from_reader` | 238 of 238       |
| PROP chunks read                                              | 52,352 of 52,352 |
| records / whole objects / deletions                           | 23,047 / 582 / 0 |
| distinct paths, longest, with a `{key}` subscript             | 124, 48 bytes, 0 |
| records that resolve, and of those, that create the leaf      | 22,877 / 2,464   |
| skipped, no such object                                       | 101              |
| skipped, an intermediate property is absent                   | 69               |
| skipped for any other reason                                  | 0                |

The last row is the assertion the test makes, and it is what [section 3.1](#s3.1) measured
independently: a shipped record never mismatches a type, subscripts something unsubscriptable, runs
off the end of a container or walks into a null pointer. The "no such object" count is lower than
[section 3.1](#s3.1)'s 173 because that measurement resolved only against the `uibase` bin in the
same directory, where this one reaches every object in the archive.
