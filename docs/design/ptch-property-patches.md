# PTCH property patches and property paths in `ltk_meta`

Design for review. Covers [#172](https://github.com/LeagueToolkit/league-toolkit/issues/172)
(PTCH data override parser) and [#173](https://github.com/LeagueToolkit/league-toolkit/issues/173)
(PTCH pointer syntax on regular bin objects).

Status: phase 1 implemented on `feat/ptch-bins`, 2026-08-22. Phases 2 and 3 are still design
only. Section 14 records where the implementation departed from this document.

## 1. Summary

A `PTCH` file is a patch applied over exactly one base `PROP` bin. After a short header it carries
three things: a set of object hashes to delete, whole objects to add, and a list of property-patch
records. Each record names one object of the base by hash, one property inside that object by a typed
path string (`Position.UIRect.Size`, `Elements[3]`, `PerAttachmentMaterial{"weapon"}`), and a value.
The client resolves the path against its reflection tables and writes the value in place.

Riot's loader calls the file a data override, which is where `BinOverride` gets its name; this
document calls it a patch bin, and the records inside it patch records, the way the magic, ritobin
and LtMAO do.

Riot ships 238 of these files, all UI scene variants (flipped minimap, mobile and tablet layouts,
mirrored scoreboard), carrying 23,047 records against 582 whole objects. `ltk_meta` today stops at
the PTCH header: it mis-reads the header's count field, does not read the records, and
`Bin::to_writer` has a `todo!()` for override bins.

The proposal lands in three phases, one PR each:

1. **Container and records** (#172). New `BinOverride` and `PropertyPatch` types with
   `from_reader` / `to_writer`, a validated `PropertyPath` newtype with a tokenizer,
   `BinFile` / `BinKind` for "read whichever kind of bin this is", and removal of
   `is_override` / `data_overrides` from `Bin`. Breaking for `ltk_meta`.
2. **Resolution and apply** (#173). `resolve`, `resolve_mut` and `patch` on `BinObject`, `Struct`,
   `Bin` and `PropertyValueEnum`, plus `BinOverride::apply` / `check` with a report that mirrors the
   client's skip semantics.
3. **ritobin text** (`ltk_ritobin`). `patches` and `deleted` root entries, byte-compatible with
   moonshadow's ritobin output for every shipped file.

Non-goals: a class schema (the resolver works on the serialized value tree, not on Riot's meta
classes), registering patches in a running client, a CLI.

## 2. Evidence

Two sources, plus a scan of the installed client:

- **The client's bin loader, decompiled.** Record layout, the version gates, the delete list,
  dependency handling, the type rule, the resolver and the apply loop. Appendix A lists the
  functions this rests on, with addresses for a 16.14-era and a 16.16 build; the statistics
  behind it came from 16.14 and 16.15 clients.
- **`PropertyPath.hpp`.** `PropertyManager::ResolvePropertyString` and `PropertyPathIterator`,
  which pin the path grammar and the `{key}` subscript as JSON text.

### 2.1 Verified on the installed client

Client 16.16.804.9184, `UI.wad.client`, scanned on 2026-08-22 with a scratch reader built on
`ltk_wad` and `ltk_meta` (not committed):

| measurement | result |
|---|---|
| PTCH chunks | 237 (the 238th lives in `Bootstrap.windows.wad.client`) |
| parse with the layout in section 3, to the exact end of the chunk | 237 of 237, zero trailing bytes |
| records / whole objects | 22,899 / 582 |
| `payloadSize` equals `1 + 2 + pathLen + value size`, value read by `PropertyValueEnum::from_reader` | 22,899 of 22,899 |
| outer version 1, inner version 3, delete count 0, dependency count 0 | all 237 |
| paths containing `{` | 0; longest path 48 bytes; all ASCII |
| record kinds | Link 3911, Vec2 3642, Embed 3495, Pointer 2885, Bool 2695, U16 2511, U32 1813, String 616, U8 417, Vec4 356, F32 325, Color 151, List2 80, Map 1, Option 1 |

Every record was then resolved against the `uibase` bin of the same directory, walking
`ltk_meta`'s value tree with the rules proposed in section 8:

| outcome | records | meaning |
|---|---|---|
| leaf exists, same shape | 20,203 | plain replace |
| leaf absent, parent exists | 2,455 | the base object omits the property (class default). The client still applies the record because the property exists on the class, so apply must insert at the leaf. |
| object absent from base and patch | 173 | the client skips (binary-search miss). Stale records, or a variant patched over a base other than `uibase`. |
| intermediate segment absent | 68 | for example `ClickedStateElements.DisplayElementList` when the base object omits `ClickedStateElements`, an Embed. The client applies into the default-constructed embedded struct; without the class we cannot. Skip and report. |
| kind mismatch, index out of range, subscript on a non-container, null pointer | 0 | |

No shipped patch object collides with a base object, so replace-on-collision is unattested either
way.

### 2.2 Worked example

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

## 3. Wire format (normative)

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
appendix A:

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

What `ltk_meta` does today, for contrast:

- `Bin::from_reader` reads `deleteCount` as `_maybe_override_object_count` and expects `PROP`
  immediately. A file with deletions fails with `InvalidFileSignature`.
- After the objects it reads `patchCount` and pushes `()` that many times without consuming a
  byte, then returns success.
- `Bin::to_writer` panics on `is_override`.
- `ltk_ritobin` reports "Patch bins are not supported yet".

## 4. Data model

Module layout in `crates/ltk_meta/src`:

```text
lib.rs                    pub mod path; mod data_override; mod file; explicit re-exports
path.rs                   PropertyPath, Segment, Segments, Subscript, KeyLiteral, PropertyPathError
path/parse.rs             tokenizer                               (phase 1)
path/resolve.rs           resolver, ResolveError, PatchError, ValueShape (phase 2)
data_override.rs          BinOverride, BinOverrideBuilder, PropertyPatch
data_override/read.rs     from_reader
data_override/write.rs    to_writer
data_override/apply.rs    apply, check, ApplyReport               (phase 2)
file.rs                   BinFile, BinKind
tree/read.rs              object-table reader shared by Bin and BinOverride
```

Visibility follows M-SINGLE-ITEM-PATH: `data_override` and `file` are private modules whose types are
re-exported from the crate root, the same arrangement as `tree`; `path` is a public module and its
items are not re-exported.

### 4.1 `BinOverride`

```rust
/// A `PTCH` patch: deletions, added objects and property patch records applied over one base bin.
#[derive(Debug, Clone, PartialEq, Default)]
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
impl<M: Default> BinOverride<M> {
    pub fn from_reader<R: Read + Seek + ?Sized>(reader: &mut R) -> Result<Self, Error>;
}
impl<M: Clone> BinOverride<M> {
    pub fn to_writer<W: Write + Seek + ?Sized>(&self, writer: &mut W) -> io::Result<()>;
}
impl<M> BinOverride<M> {
    /// Consumes the patch: objects and values move into the base, nothing is cloned (D17).
    pub fn apply(self, base: &mut Bin<M>) -> ApplyReport;    // phase 2
    pub fn check(&self, base: &Bin<M>) -> ApplyReport;       // phase 2
}
```

No version fields: the client accepts exactly one value for each (D2); an older inner version is
read and written back as 3, as `Bin` does (D15). No dependencies: the client cannot
load a patch that declares any (D3). `deleted` is a `Vec`, not a set, so a file round-trips
byte-for-byte. Fields are public for parity with `Bin` and `BinObject`.

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

`build()` is infallible: the only fallible piece, `PropertyPath::new`, is validated before it
enters the builder, so M-BUILD-RESULT has nothing left to check.

### 4.2 `PropertyPatch`

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

### 4.3 `PropertyPath`

A validated newtype over `String` (C-NEWTYPE, M-STRONG-TYPES-GUARD): every `PropertyPath` is
well-formed per section 7. The text is preserved byte-for-byte (casing, `0x` indices, whitespace
inside a key), so a read file round-trips exactly. `PartialEq`, `Hash` and `Ord` are textual; use
`segments()` and `Segment::name_hash` for a case-insensitive comparison.

```rust
pub struct PropertyPath(String);

impl PropertyPath {
    /// Wire limit: `pathLen` is a `u16`.
    pub const MAX_LEN: usize = u16::MAX as usize;

    pub fn new(path: impl Into<String>) -> Result<Self, PropertyPathError>;
    pub fn as_str(&self) -> &str;
    pub fn segments(&self) -> Segments<'_>;   // Iterator<Item = Segment<'_>> + Clone

    // In-place extension like `PathBuf::push`; each call validates the new piece.
    pub fn push_field(&mut self, name: &str) -> Result<(), PropertyPathError>;
    pub fn push_index(&mut self, index: u32) -> Result<(), PropertyPathError>;      // renders decimal
    pub fn push_key(&mut self, key: &KeyLiteral<'_>) -> Result<(), PropertyPathError>; // renders JSON
}
// Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Display (the text), FromStr,
// AsRef<str>, Borrow<str>, TryFrom<&str>, TryFrom<String>,
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

The type lives in `ltk_meta::path`, not under `data_override`, because the same language is used by Riot's
tools for deep pathing and by some property values at runtime, and #173 asks for it on regular
bin objects. The patch record is one consumer.

### 4.4 `BinFile` and `BinKind`

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

## 5. Changes to `Bin`

Breaking, `feat(meta)!`, `ltk_meta` 0.6.1 to 0.7.0:

- `Bin::is_override` and the private `data_overrides` field are removed, and so is
  `Builder::is_override`. `Bin` is the `PROP` type only.
- `Bin::from_reader` on a `PTCH` magic returns
  `Error::UnexpectedBinKind { expected: Prop, found: Override }` instead of reading half the file.
- `Bin::to_writer` loses the `todo!()` branch and always writes `PROP`.
- `Bin::PROP` and `Bin::PTCH` become `BinKind::magic()`.
- The object-table reader is factored out of `Bin::from_reader` so `BinOverride` shares it, including
  the legacy-kind retry (D6).

Known downstream use: `crauzer-ritobin-lsp` reads `bin.is_override` in three places.

Variants added to `ltk_meta::Error`:

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
Resolve(#[from] ResolveError),   // phase 2, so `?` works in code that mixes I/O and resolution
```

## 6. Reading and writing rules

`BinOverride::from_reader`:

1. Magic `PTCH`. A `PROP` magic gives `UnexpectedBinKind`; anything else `InvalidFileSignature`.
2. Outer version must be 1, else `InvalidOverrideVersion`.
3. `deleteCount` hashes are read into `deleted`. This is the header bug fix.
4. Inner magic must be `PROP` (`InvalidFileSignature`). Inner versions 1 to 3 are accepted, as
   for `Bin` (D15): the dependency count exists from version 2 and the record list only in
   version 3, the gate LtMAO uses too. Anything else is `InvalidFileVersion`.
5. `dependencyCount` (version 2 and up) must be 0, else `OverrideDependencies` (D3).
6. Objects go through the reader shared with `Bin`: class table, sized objects, `InvalidSize`
   enforcement, legacy retry (D6).
7. Records: `objectHash`, `payloadSize`, then the body under `ltk_io_ext::measure`, compared to
   `payloadSize` and rejected with `InvalidSize` on mismatch (the toolkit enforces the sizes the
   client ignores, consistent with objects, structs and maps). `kind` goes through
   `Kind::unpack(raw, legacy)`, the path through `read_sized_string_u16` and `PropertyPath::new`
   (`InvalidPropertyPath`, D4), the value through `PropertyValueEnum::from_reader(reader, kind, legacy)`.
8. No end-of-stream check, same as `Bin`.

`BinOverride::to_writer` always emits `PTCH`, `1`, `deleted`, `PROP`, `3`, `0`, the class table and objects
exactly as `Bin` does, `patches.len()`, then per record `objectHash`, a size placeholder, the body under
`measure`, and the size back-patched with `window_at`. For every shipped file the output is
byte-identical to the input (section 10).

## 7. Property path grammar

### 7.1 Grammar (normative for `PropertyPath::new`)

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

| path | parses to | note |
|---|---|---|
| `Enabled` | field | |
| `Position.UIRect.Size` | field, field, field | |
| `Elements[3]` | field + Index(3) | |
| `AnimationItems[0x1].SpeedScale` | Index(1), then field | hex accepted, text preserved |
| `PerAttachmentMaterial{"weapon"}` | field + Key(String("weapon")) | |
| `Lookup{ 12 }` | field + Key(Number("12")) | JSON whitespace allowed around a key |
| (empty string) | error `EmptySegment` | |
| `Position.` or `.Position` | `EmptySegment` | |
| `Elements[3]x` | `UnexpectedCharacter('x')` | a subscript must end the segment |
| `Elements[-1]`, `Elements[ 3 ]`, `Elements[3.0]` | `InvalidIndex` | |
| `Map{null}`, `Map{[1]}` | `InvalidKey` | only scalars can convert to a key kind |
| `A[1][2]`, `A[1]{2}` | `DoubleSubscript` | one subscript per segment; the format has no nested containers |
| `A[(1]` | `UnbalancedBracket` | |

### 7.2 Where we are stricter than the client

The client's tokenizer never rejects a path; bad paths fail at resolution and the record is skipped.
`PropertyPath::new` rejects: unbalanced brackets, empty names, parentheses and control characters in
names (the client only counts parentheses for nesting), indices that are not a complete
non-negative `strtol` token, keys that are not a JSON scalar, and paths longer than `u16::MAX`.
Every such path is dead weight the game would silently ignore, and the toolkit already rejects
other things the client tolerates (size mismatches). All 23,047 shipped records pass.

The path is copied into a 512-byte stack buffer, but the retail limit is not measured and shipped
paths top out at 48 bytes, so the limit is documented, not enforced.

### 7.3 `PropertyPathError`

```rust
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{kind} at byte {offset}")]
pub struct PropertyPathError { offset: usize, kind: PropertyPathErrorKind }
impl PropertyPathError {
    pub fn offset(&self) -> usize;
    pub fn kind(&self) -> PropertyPathErrorKind;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

## 8. Resolution and apply (phase 2)

### 8.1 API

```rust
impl<M> BinObject<M> {
    pub fn resolve(&self, path: &PropertyPath) -> Result<&PropertyValueEnum<M>, ResolveError>;
    pub fn resolve_mut(&mut self, path: &PropertyPath) -> Result<&mut PropertyValueEnum<M>, ResolveError>;
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
    pub fn resolve_mut(&mut self, path: &PropertyPath) -> Result<&mut PropertyValueEnum<M>, ResolveError>;
}
impl<M> Bin<M> {
    pub fn resolve(&self, object_hash: impl Into<BinHash>, path: &PropertyPath)
        -> Result<&PropertyValueEnum<M>, ResolveError>;
    pub fn resolve_mut(&mut self, object_hash: impl Into<BinHash>, path: &PropertyPath)
        -> Result<&mut PropertyValueEnum<M>, ResolveError>;
    pub fn patch(&mut self, object_hash: impl Into<BinHash>, path: &PropertyPath, value: PropertyValueEnum<M>)
        -> Result<Option<PropertyValueEnum<M>>, PatchError>;
}
```

Everything is inherent (M-ESSENTIAL-FN-INHERENT), receivers are plain references
(M-AVOID-WRAPPERS), and `M` stays free so `ltk_ritobin` can resolve over `PropertyValueEnum<Span>`.

`resolve_mut` is the raw escape hatch: it gives `&mut` to whatever is there and performs no type
check. `patch` is the client-faithful operation.

### 8.2 Traversal rules

| value at the cursor | next piece | result |
|---|---|---|
| object, Struct with class != 0, Embed | `.name` | property by `name_hash`; absent: `MissingProperty` (`patch` inserts when it is the last segment) |
| Struct with class 0 | anything | `NullPointer` |
| List, List2 | `[i]` | element `i`; `i >= len`: `IndexOutOfRange` |
| Option | `[0]` | the contained value; absent or `i > 0`: `IndexOutOfRange` |
| Map | `{k}` | the entry whose key equals `k` converted to the key kind; literal not convertible: `InvalidKey`; no entry: `KeyNotFound` |
| List, List2, Option, Map | `.name` | `CannotDescend` |
| object, Struct, Embed | `[i]` or `{k}` | `NotIndexable` |
| any leaf kind (primitives, String, Hash, WadChunkLink, Link, Flag) | anything | `CannotDescend` or `NotIndexable` |
| element reached through a subscript | more segments | continue only if it is a Struct or Embed, else `CannotDescend` |

Link (0x84) is a leaf. The client does not follow links while resolving a path; only Pointers are
dereferenced.

### 8.3 The type rule in `patch`

The client compares the record's tag with the resolved property's registered tag; for containers
also the element tags, for Embed the exact class, and for Pointer it accepts descendants. Without a
schema the closest faithful check is against the value currently in the tree:

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
// Display: "vec2", "list2[link]", "map[hash,string]", "embed 4eb9ba4f"
```

- Leaf present: `ValueShape::of(existing).matches(&ValueShape::of(&value))`, otherwise
  `PatchError::TypeMismatch { expected, found }` and nothing changes.
- Leaf absent and the parent is an object, Struct or Embed: insert. 2,455 shipped records rely on
  this. No check is possible; the documentation says the client still drops the value if the kind
  does not match the class.
- A Pointer value replaces the existing one wholesale whatever its class, which is what the client
  does for an accepted descendant.

### 8.4 Errors

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
pub enum PatchError {
    #[error(transparent)]
    Resolve(#[from] ResolveError),
    #[error("type mismatch: the property is {expected}, the patch carries {found}")]
    TypeMismatch { expected: ValueShape, found: ValueShape },
}
```

### 8.5 `BinOverride::apply` and `check`

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

## 9. ritobin text (phase 3)

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

## 10. Testing

Fixtures, added under `crates/ltk_meta/tests/bins/` next to `leona_small.bin`:

- `lolminimap_uiflipped.ptch.bin`, 5762 bytes, 0 objects, 109 records (the flipped minimap).
- `lolminimap_uibase.bin`, 18,256 bytes, 66 objects (its base).

Both come from `UI.wad.client` of client 16.16.804.9184.

Phase 1:

- Tokenizer table: every row of section 7.1, plus `push_field` / `push_index` / `push_key`,
  `Display` and `FromStr` round trips, the serde string form.
- Round trips through `to_writer` then `from_reader` for a builder-made patch covering every value
  kind, mirroring `tree/tests.rs`.
- The fixture reads, re-writes byte-identical, and its parsed form is an insta `.ron` snapshot.
- Error paths: `PROP` magic into `BinOverride::from_reader`, `PTCH` magic into `Bin::from_reader`,
  outer version 2, inner version 2, one dependency, a truncated record, a bad path.

Phase 2:

- A synthetic tree that exercises every row of section 8.2 and every `ResolveErrorKind`.
- `check(uiflipped, uibase)` as a snapshot: 109 applicable, the insert count, 0 skipped.
- `apply` then `resolve` spot checks: `VoiceChatPanel_ButtonClicked` anchor goes from `(1, 1)` to
  `(0, 1)`, `MinimapFrame.FlipX` appears as `true`.
- Corpus test, `#[ignore]`, enabled by `LTK_LOL_GAME_DIR`: every PTCH chunk in the install reads,
  re-writes byte-identical, and `check` against its `uibase` reproduces the numbers in section 2.1.

Phase 3: print and parse snapshots for the fixture; diagnostics for a non-empty `linked` on a
`PTCH`, a `patches` entry that is not a `patch`, and a `path` that fails `PropertyPath::new`.

## 11. Guideline review

| item | how the design meets it |
|---|---|
| C-NEWTYPE, M-STRONG-TYPES, M-STRONG-TYPES-GUARD | `PropertyPath` guards its grammar at construction; fallible constructors only, no `From<&str>` |
| C-BUILDER, M-INIT-BUILDER, M-BUILD-RESULT | `BinOverrideBuilder`, chainable, infallible `build()` because inputs are pre-validated |
| C-COMMON-TRAITS, C-SERDE, M-PUBLIC-DEBUG, M-PUBLIC-DISPLAY | the usual derives on every public type; `PropertyPath`, `Segment`, `ValueShape`, `ApplyReport` and the errors implement `Display`; serde behind the existing feature |
| C-CONV, C-GETTER, C-ITER, C-ITER-TY | `as_str`, `segments()` returning `Segments`, `kind()`, `offset()` |
| C-GOOD-ERR, M-FROM-ERROR | errors implement `Error` + `Display`, carry the failing offset or segment, and convert into `ltk_meta::Error` through `From` |
| C-VALIDATE | sizes, versions, dependency count and path syntax are checked on read |
| C-CALLER-CONTROL | `apply(self, base)` moves objects and values into the base; cloning a patch to reuse it is the caller's explicit choice |
| C-RW-VALUE | `from_reader(&mut R)` / `to_writer(&mut W)` keep the crate's existing signature shape (the constitution fixes it) |
| M-ESSENTIAL-FN-INHERENT, M-AVOID-WRAPPERS, M-SIMPLE-ABSTRACTIONS | inherent methods on plain `&self` / `&mut self`; one type parameter (`M`), no nesting |
| M-SINGLE-ITEM-PATH, M-NO-GLOB-REEXPORTS | new modules use explicit re-exports; each new item is reachable through one path |
| M-PANIC-ON-BUG, M-PANIC-IS-STOP | no `todo!()` or `unwrap` in library paths (the `to_writer` panic goes away) |
| M-LOG-NOT-PRINT | the legacy retry keeps using `log::warn!`, nothing prints |

Where the two references or the crate disagree:

- **M-ERRORS-CANONICAL-STRUCTS vs the crate's enum.** The Microsoft guideline wants
  situation-specific error structs with a private kind. `ltk_meta::Error` is a public `thiserror`
  enum and stays one (crate convention; C-GOOD-ERR is satisfied either way). The new
  situation-specific errors, `PropertyPathError` and `ResolveError`, are structs with a position
  and a kind, but the kind enum is public rather than hidden behind `is_xxx()` helpers, because
  callers classify skips in `ApplyReport`.
- **M-INIT-BUILDER naming.** The guideline wants `FooBuilder`; the crate exposes `Bin`'s builder as
  `Builder`. The new one is `BinOverrideBuilder`. Renaming `Builder` to `BinBuilder` is a separate
  cleanup.
- **C-STRUCT-PRIVATE.** `Bin` and `BinObject` expose their fields; `BinOverride` and `PropertyPatch`
  do the same for consistency. `PropertyPath` keeps its field private because it carries an
  invariant.

## 12. Decisions

Decisions, each with the alternative that was not taken:

- **D1 Name.** `BinOverride` for the file, the game's own wording (`BinFileCache_addDataOverride`,
  `PropertyOverrideLoadable`, the `cache->overrides` list), and the one `ltk_file` already uses
  (`LeagueFileKind::PropertyBinOverride`). The records stay `PropertyPatch` and the per-property
  verb stays `patch()`: the magic is `PTCH`, LtMAO (`is_patch`, `patches`) and ritobin (`patches`)
  call them patches, and `override` is a reserved keyword in Rust, so it cannot name a module or a
  method without `r#`. The private module is `data_override`.
- **D2 No version fields.** Both are fixed by the client; the reader rejects anything else, the
  writer emits constants. `Bin` keeps `version` only because v2 files exist in the wild.
- **D3 Dependencies are rejected on read** (`OverrideDependencies`) and not representable. A patch
  with dependencies cannot load in the client. Alternative: read and keep them, error on write.
- **D4 Path syntax is validated on read** (`InvalidPropertyPath`). Alternative: keep a raw
  `String` in the record and validate at resolve time. The typed form is what #173 is about, and
  all shipped records pass.
- **D5 `payloadSize` is enforced** like every other size field in the crate.
- **D6 Legacy kind numbering** (pre-`WadChunkLink` tags) is handled by the shared object reader's
  retry. Record kinds are read as non-legacy with no retry: records only exist in inner version 3,
  which postdates the renumbering, so a legacy record list cannot exist. One that somehow did would
  fail with `InvalidPropertyTypePrimitive` rather than be misread.
- **D7 A patch object whose hash exists in the base replaces it**, reported in
  `ApplyReport::replaced`. The client would hold both in its merged table with unspecified lookup
  order. Unattested in shipped data.
- **D8 A missing intermediate segment is a skip** (`MissingProperty`). A schema hook for
  default-constructing an absent Embed can come later as `apply_with(&self, base, &ApplyOptions)`
  without breaking anything.
- **D9 `Option[0]`** is supported, per the 16.14 decompile of `MetaPath_resolve`. Untested
  in-game.
- **D10 `{key}`** follows `PropertyPath.hpp`. Untested in-game: zero shipped records.
- **D11 The type rule** compares `ValueShape`: kind, item and key kinds, Embed class exact,
  Pointer class ignored. Corrected on 2026-08-24: the client does compare a Pointer's class,
  accepting any that derives from the declared one and skipping the rest. Deciding that needs the
  class hierarchy, which only the game has, so the class stays out of the comparison and this
  crate accepts a Pointer the client might reject. See section 15.3.
- **D12 `BinFile`** ships in phase 1. It is small and it is what every scanning tool wants.

Resolved in review on 2026-08-22:

- **D13 Paths are object-relative.** A `PropertyPath` addresses a property inside one object; the
  object is a separate argument (`Bin::resolve(object_hash, path)`). Addressing a whole file with one
  string can come later as an extension of the grammar without touching the object-level API.
- **D14 The ritobin root for the delete list is `deleted: list[hash]`**, omitted when empty.
- **D15 All file versions stay readable.** The toolkit has always read every version it knows:
  `Bin::from_reader` keeps accepting `PROP` 1 to 3, and `BinOverride::from_reader` accepts inner
  versions 1 to 3 (the record list is read only for 3). Both writers emit 3. The client's gates
  are documented in section 3, not enforced on read. D3 is unaffected: dependencies are not a
  version, and a patch that has any is unloadable on every client.
- **D16 `ValueShape` keeps its name.** Building `ltk_ritobin::RitoType` on it is a follow-up
  once both exist.
- **D17 `apply` consumes the patch.** `apply(self, base)` moves objects and values, clones
  nothing and needs no `M: Clone`. `check(&self, base)` borrows. Reusing a patch is
  `patch_bin.clone().apply(...)`.
- **D18 Fixtures are real game files.** The two files in section 10 are small enough to live in
  the repo next to `leona_small.bin`.

## 13. Implementation plan

| PR | commit subject | contents | versions |
|---|---|---|---|
| 1 | `feat(meta)!: read and write PTCH bins` | `path` grammar and tokenizer, `BinOverride`, `PropertyPatch`, `BinOverrideBuilder`, `BinFile`, `BinKind`, `Bin` cleanup, shared object reader, fixtures, tests, `lib.rs` examples, `LTK_GUIDE.md` | `ltk_meta` 0.7.0 |
| 2 | `feat(meta): resolve property paths and apply PTCH patches` | resolver, `patch`, `ValueShape`, `apply` / `check`, `ApplyReport`, corpus test | `ltk_meta` 0.7.x or 0.8.0, release-plz decides |
| 3 | `feat(ritobin)!: read and print PTCH bins` | `Patches` / `Deleted` roots, `Cst::build`, `Print for BinOverride`, diagnostics, snapshots | `ltk_ritobin` next minor |

Follow-ups outside this repo: `crauzer-ritobin-lsp` (`is_override` removal, `BinFile`), bin-grep,
a `ptch check` command in whichever CLI wants it.

## Appendix A. Client functions

16.14-era exe, image base `0x140000000`:

| address | name |
|---|---|
| `0x1411B3BC0` | `ReadPropertyHeader_unk`, base and patches in lock-step |
| `0x1411B3450` | `PropertyPatch_readAndApply`, one record |
| `0x1411B7730` | `MetaPath_resolve`, the path language |
| `0x1411B4DD0` | `MetaValue_readInto` |
| `0x1411B87B0` | `MetaValue_skipByType` |
| `0x1411A95E0` | `MetaClass_adjustPointerTo` |

16.16.8042073: `MetaFile_readImpl` at `0x1411B6D90` (the version gates and the delete list),
`MetaFile_readEntry` at `0x1411B7940` (the delete set applied per entry).

## Appendix B. Scratch tooling

Three throwaway crates produced the numbers in section 2, built against the workspace crates on
this branch. They live in the session scratchpad and are not part of the change:
`ptchdump` (layout and statistics), `ptchresolve` (resolution outcomes against `uibase`),
`ptchchain` (the worked example). The corpus test in section 10 is their permanent replacement.

## 14. Implementation notes (phase 1)

Phase 1 landed as `feat(meta)!: read and write PTCH bins`. What the code does differently from
the sections above, and why:

- **`from_reader` is not generic over `M`.** Rust does not apply a struct's default type parameter
  in expression position, so `BinOverride::from_reader(&mut r)` on an `impl<M: Default>` block
  fails to infer `M` and would need a turbofish at every call site. `Bin::from_reader` already
  sidesteps this by living on `impl Bin`, and `BinOverride::from_reader` and `BinFile::from_reader`
  now do the same. `to_writer` has no such problem because the receiver pins `M`, so it is
  `impl<M: Clone>` on all three, and `Bin::to_writer` was widened from `Bin<NoMeta>` to
  `Bin<M: Clone>` so `BinFile<M>` can forward to it. Making every reader generic is a coherent
  follow-up for the whole crate, not something to do for one type.
- **`Default` is `BinOverride<NoMeta>` only**, for the same inference reason and the same way `Bin`
  does it. `new()` stays on `impl<M>`.
- **`push_field` rejects a name that is not one segment.** `push_field("A.B")` would otherwise
  append two, which is not what the method says it does.
- **The legacy-kind retry now rewinds.** The existing retry re-read the object table from wherever
  the first attempt failed, mid-object, so it could never have worked; the shared reader records
  the position before the table and seeks back. The record list has no retry at all (D6).
- **The builder's bulk deletion method is `deletions`**, next to `objects` and `patches`.
- Small additions the design did not list: `PropertyPath::len` / `is_empty` and
  `From<PropertyPath> for String`, `From<&str>` and `From<bool>` for `KeyLiteral`, `From<Bin<M>>`
  and `From<BinOverride<M>>` for `BinFile<M>`. `BinKind::identify_from_bytes` /
  `identify_from_reader` and the `BinFile` accessors in section 4.4 came out of review.
- **The `uibase` fixture is deferred to phase 2**, where the resolver first needs it. Only
  `lolminimap_uiflipped.ptch.bin` is in the repo so far.
- **The fixture snapshot covers the first three records**, not the whole patch: a 2,500 line
  snapshot is not reviewable, and the byte-identical rewrite already fails on any misparse.
- `serde_json` is a new dev-dependency, for the `PropertyPath` serde round trip.

### 14.1 Corpus check

Not a committed test (the corpus test in section 10 is phase 2 work); run from a scratch crate
against client 16.16.804.9184, every `.wad.client` in the install:

| measurement | result |
|---|---|
| PTCH chunks read, re-written and compared byte for byte | 238 of 238 exact, 0 mismatches, 0 failures |
| `BinFile::from_reader` agrees with `BinOverride::from_reader` | 238 of 238 |
| PROP chunks still read after the reader refactor | 52,352 of 52,352 |
| records / whole objects / deletions | 23,047 / 582 / 0 |
| distinct paths, longest, with a `{key}` subscript | 124, 48 bytes, 0 |

## 15. Implementation notes (phase 2)

### 15.1 Container storage

Section 8.1 hands out `&PropertyValueEnum<M>`, which `Container`, `UnorderedContainer` and
`Optional` could not do: they stored typed variants (`Vec<values::Vector2<M>>`,
`Option<values::I32<M>>`), so an item was never a `PropertyValueEnum` to lend out. `Optional` had
no borrowing accessor at all. `Map` had stored `PropertyValueEnum` pairs all along.

The two now match `Map`: an `item_kind: Kind` beside `Vec<PropertyValueEnum<M>>` (`Optional` boxes
its single value, because the type allows the nesting the format forbids). The homogeneity the
variants enforced at compile time is enforced at run time by the constructors and `push`, which is
where `Map` already enforced it and where `Container::push` already did; `items_mut` and
`value_mut` are documented as the way around it. `ContainerItem`, a marker for the value types the
format lets a container hold, keeps `From`/`FromIterator` rejecting a nested container at compile
time.

This deleted `container/iter.rs` and the `container_variants!` list, and cost eight lines in
`ltk_ritobin`. It landed as `refactor(meta)!: store container and option items as
PropertyValueEnum`, before the resolver.

### 15.2 `ValueSlot`

Flattening the containers opened a hole the typed variants had closed: a `&mut PropertyValueEnum`
into a container could be assigned a value of another kind, leaving the container disagreeing with
its own declared item kind and the writer emitting a file the game cannot read, silently.

`ValueSlot` closes it. A mutable borrow is used for two different things and only one is
dangerous: replacing the whole value can change the kind, editing inside it cannot. So no `&mut
PropertyValueEnum` is handed out at all. `Container::items_mut`, `Optional::value_mut` and the
`Map::entries_mut` that briefly existed are replaced by `slot`, which returns a handle carrying
the kind its holder pins it to. `ValueSlot::set` checks that kind; `ValueSlot::as_mut` and
`ValueSlot::get_mut` reach the concrete value type, where the kind is not expressible as anything
else. A slot on an object or struct property pins nothing, because there the kind is free.

Two supporting pieces on `PropertyValueEnum`, both generated from the existing variant list and
useful on their own: `ValueMut`, a borrowed enum with one variant per kind, and `FromValue`,
behind `get` and `get_mut`. The crate had no `as_*` accessors at all before this, so reaching an
`i32` meant writing a `match`.

This changed `resolve_mut`'s return type from section 8.1's `&mut PropertyValueEnum<M>` to
`ValueSlot<'_, M>`. Section 8.1 called `resolve_mut` the raw escape hatch that performs no type
check, which was written when containers held typed variants and the hole did not exist.

### 15.3 What the client reference changed

The resolver was written from sections 8.2 and 8.3, which were paraphrased from the decompile
during phase 1. Reading the source documents again before implementing confirmed the traversal
table and the outer type gate, and changed two things.

- **D11 was wrong.** The client's pointer reader walks the primary base chain and then the
  secondary-base pairs, so a class that derives from the declared one is accepted and constructed
  as the file's class, and an unrelated or unresolvable class is skipped. It is an is-a test, not
  an absence of one. Without the class hierarchy there is no way to run it, so `ValueShape` leaves
  a pointer's class out entirely, which accepts strictly more than the client does. An Embed is
  unaffected: the client compares `MetaClass` pointers, so the class must be exact.
- **Insertion has no client counterpart at all.** The client patches a live object that was
  in-place constructed from its class before deserialization, so every property the class declares
  already exists at its offset holding whatever its constructor left there; the file only
  overwrites the ones it carries. There is no absent leaf and no insert. Both halves of D8 are
  therefore toolkit decisions with nothing to check them against, and the skip half does strictly
  less than the game: patching into an intermediate Embed the base never serialized is something
  the client handles without noticing.

Two smaller confirmations worth recording. Indices are parsed with `strtol` base 0, so `[0x1F]`
and `[010]` are legal, which section 7.1 already accepts. And a `Link` is never dereferenced while
resolving: no descent rule exists for it, and every Link-typed path in the shipped corpus is
terminal.

The `{key}` conversion follows `PropertyPath.hpp`, which has the brace text parsed as JSON and
coerced to the map's key type. The other source states flatly that the resolver touches no JSON at
all. Neither can be settled from shipped data, because no shipped record uses a `{key}` subscript
(D10). `PropertyPath.hpp` also resolves a quoted string against a global enum registry for an
enum-typed key, which needs a schema and is not implemented.

`ValueShape`'s `Display` uses this crate's own `Kind` names rather than ritobin's - `Vector2`,
`Container[I32]`, `Map[Hash, String]`, `Embedded 4eb9ba4f`. The ritobin vocabulary lives in
`ltk_ritobin` and duplicating it here would give `ltk_meta` two names for every kind.

`check` differs from `apply` in one way the design did not call out: it judges every record
against the base as it stands, where `apply` runs them in file order. A record that only fits
because an earlier record in the same patch replaced a pointer or an embed above it is therefore
judged against the value that earlier record would have overwritten.

### 15.4 Corpus check

The scratch tooling in appendix B is replaced by `crates/ltk_meta/tests/corpus.rs`, ignored unless
`LTK_LOL_GAME_DIR` points at an install. Rather than the sibling-`uibase` heuristic the phase 1
scratch tool used, it collects every object a patch's records name, by hash, from the archive the
patch lives in - a bin object's path hash is the hash of its asset path, so a record lands on the
object it names without needing to know which file that is.

Against client 16.16.804.9184, 456 archives:

| measurement | result |
|---|---|
| PTCH chunks read, re-written and compared byte for byte | 238 of 238 |
| records / whole objects / deletions | 23,047 / 582 / 0 |
| records that resolve, and of those, that create the leaf | 22,877 / 2,464 |
| skipped, no such object | 101 |
| skipped, an intermediate property is absent | 69 |
| skipped for any other reason | 0 |

The last row is the assertion the test makes, and it is the one section 2.1 measured: a shipped
record never mismatches a type, subscripts something unsubscriptable, runs off the end of a
container or walks into a null pointer. The first two rows reproduce section 14.1 exactly. The
"no such object" count is lower than section 2.1's 173 because that measurement resolved only
against `uibase` in the same directory, where this one reaches every object in the archive.
