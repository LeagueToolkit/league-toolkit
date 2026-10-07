# ADR-0022: Hash width from declared sizes

- **Status:** Accepted
- **Date:** 2026-10-07
- **Crates:** `ltk_hash`, `ltk_meta`
- **Related:** #272, #273, ADR-0006, ADR-0007, ADR-0008, ADR-0009,
  `docs/design/bin-streaming.md` [section 7.1](../design/bin-streaming.md#s7.1)

## Context and problem statement

A `Hash` property value (kind byte 17) occupies 4 or 8 bytes. PBE 16.21 stores
`StaticMaterialDef.name` in 8 bytes. The kind byte is 17 at both widths. The file does not store
the width.

The client keeps each `Hash` in a `u64`. Each `Hash` property has a helper that defines the stored
size and the hash function. The client finds the helper through the class of the object.

`lol-meta-classes` records the helper per property and per build. A bin does not store its build.
The helper of a property can differ between builds: `IUiVariable` property `0x8d9a165f` used XXH3
at 4 bytes in 16.17.

A reader that reads each `Hash` as 4 bytes stops 4 bytes before the end of an 8-byte value. It
then reads a byte of the hash as the next kind byte. The read fails, or the handle latches onto
the legacy numbering.

Each `Hash` value is inside a region that declares its byte size: an object, a struct, a container
or a map. The walk of ADR-0009 compares that size with the number of bytes that the counts
consume.

## Decision drivers

- `ltk_meta` has no schema (ADR-0006). The crate receives only a file.
- A bin that is read and then written has the same bytes.
- For a file without an 8-byte `Hash`, the reader returns the same values and the same errors as a
  reader that reads each `Hash` as 4 bytes. It walks each object once.
- The owned decode and the views return the same value for the same bytes (ADR-0008).
- `Bin::from_reader`, `BinObject::from_reader` and the `ReadProperty` impls keep their signatures.

## Considered options

1. **Widths from declared sizes, 4 bytes first** - The reader walks with each `Hash` as 4 bytes. If
   the walk fails, a search finds widths such that each region ends at its declared size.
2. **A schema from the caller** - The caller passes a table that maps a class and a property to a
   width.
3. **A full search on every object** - The reader enumerates all assignments of widths. It fails on
   an object with more than one valid assignment.
4. **Widths resolved by each view** - A view resolves the widths of its own region when it is
   created. The views share no state.

## Decision

**Option 1. A `Hash` value contains its width. The reader resolves the width from the sizes that
the file declares.**

`docs/design/bin-streaming.md` [section 7.1](../design/bin-streaming.md#s7.1) specifies the rules.
The option contains these decisions:

- **The value type is `HashValue`: a `u64` and a `HashWidth`.** The client uses the same model.
  Equality compares the width. `Kind::Hash` covers both widths. No kind is added.
- **The first walk is the walk of ADR-0009 with each `Hash` as 4 bytes.** The search runs only if
  that walk fails. If the search finds no assignment, the reader returns the error of the first
  walk.
- **The search resolves each sized region independently.** The extent of a region does not depend
  on the widths inside it. All items of a container have the same width. The reader computes that
  width from the body size and the count. All keys of a map have the same width. All values of a
  map have the same width. The search tries a `Hash` property as 4 bytes first and as 8 bytes
  second, in file order, with backtracking.
- **The result is a record of the offsets of the 8-byte values. The cursor reads the record.** The
  walk that validates a buffered object writes the record (ADR-0007). The owned decode and the
  views read the same record. The record of an object without an 8-byte `Hash` is empty.
- **The search runs before the legacy-numbering retry. The search does not run with the legacy
  numbering.** A file with the legacy numbering was written before the client stored a `Hash` in 8
  bytes.
- **For a map, the search selects 4-byte keys and 4-byte values if they are valid.** The first
  walk reads the same widths. The result for a map does not depend on the other values of the
  object.
- **The reader fails with `AmbiguousHashWidth` on a map of `Hash` keys and `Hash` values with 12
  bytes per entry.** Such an entry is valid as a 4-byte key with an 8-byte value. It is also valid
  as an 8-byte key with a 4-byte value.
- **The reader reads a `Hash` as 4 bytes if no declared size contains it.** This applies to
  `values::Hash::from_reader` and to the item of an optional that is read on its own. A `PTCH`
  record declares the size of its value. The reader computes the width of a record value from that
  size.
- **The search has a step limit that is proportional to the buffer size.** One step is one
  property, one item or one map entry. If the search reaches the limit, the reader returns the
  error of the first walk.

## Consequences

- **Positive:** Every entry point reads a bin with 8-byte hashes without a class dump. The written
  bytes equal the input bytes. The corpus sweep over PBE measures this
  ([appendix D](../design/bin-streaming.md#appendix-d)).
- **Positive:** The reader walks a file without an 8-byte `Hash` once. The search does not run for
  such a file, and its record is empty.
- **Negative:** The crate returns a result that the client does not produce. The reader reads an
  8-byte `Hash` property as 4 bytes if the bytes after the first 4 bytes of the hash are also a
  valid property. This requires two conditions. The low byte of the next property name is a valid
  kind byte. The value of that kind ends at the end of the next property. If the next property has
  a 4-byte value, 5 of the 256 byte values meet both conditions. The property names of a class are
  fixed. An object is affected only if its class has such a pair of adjacent properties. The
  reader returns wrong values for an affected object. The written bytes equal the input bytes.
  `StaticMaterialDef.name` is not affected.
- **Negative:** The reader reads the 8-byte `Hash` keys of a map as 4 bytes if the body of the map
  also ends at its declared size with 4-byte keys. This requires a value without a fixed width on
  the other side of the map. The same applies to 8-byte `Hash` values and a key without a fixed
  width.
- **Negative:** `PropertyValueEnum::from_reader` and the `ReadProperty` impls read to the declared
  end of a value before they return an error from inside the value. A 4-byte reader returns such
  an error after the bytes that it has read.
- **Negative:** The reader accepts some damaged files that a 4-byte reader rejects. It accepts a
  file that fails with 4-byte widths and is valid with another assignment.
- **Negative:** The writer can write a map that the reader does not read: `Hash` keys of one width
  with `Hash` values of the other width.
- **Negative:** These changes break the public API:
  - `values::Hash::value`, `ValueView::Hash`, `Leaf::Hash` and `MapKey::Hash` contain a
    `HashValue`. `values::Hash` implements `Deref` and `AsRef` for `HashValue`.
  - `Display` and `LowerHex` of a `Hash` value write 8 or 16 digits with leading zeros.
  - `Kind::fixed_width()` returns `None` for `Kind::Hash`.
  - The checked constructors of `Container` and `Map` fail on a second hash width. The writer
    fails on a container or a map with two hash widths.
  - `Bin::merge` replaces a map whole if the hash widths of its keys or of its values differ from
    the base. `Replaced::mismatched` is `true` for two `Hash` values with different widths.
  - In a serde format that is not human-readable, a `Hash` value is the tuple of a `u64` and the
    width in bytes.
- **Revisit when:** A shipped class has the layout of the first negative consequence, or the
  client reads the width from data that the file contains. In the first case, option 2 can
  supplement option 1: the caller passes widths, and the reader tries them before 4 bytes.

## Pros and cons of the options

### Option 2: a schema from the caller

- Good: The width of each property is exact, as in the client.
- Bad: The crate cannot read a file without a dump of the build that wrote the file. A bin does
  not store its build. A table of another build contains a wrong width for a property whose
  helper differs.
- Bad: Every entry point needs a parameter, including `ReadProperty::from_reader`.
- Bad: The dump contains no helper for a `Hash` item of a container, a map or an optional.

### Option 3: a full search on every object

- Good: The reader fails on an object with two valid assignments. It does not return wrong values
  for such an object.
- Bad: The reader walks each object more than once. The number of assignments increases with the
  number of `Hash` properties. A file without an 8-byte `Hash` has this cost too.
- Bad: The reader fails on every object of a class with the ambiguous layout.

### Option 4: widths resolved by each view

- Good: A cursor needs no record and no offset of its buffer.
- Bad: A property iterator must resolve its region before it returns the first property. A lookup
  that returns at the first match resolves the whole region on each call.
- Bad: The view and the validating walk resolve widths separately. Their results are equal only
  if both run the same search. A view does not validate the regions inside its region.
