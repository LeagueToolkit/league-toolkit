# Inibin files in `ltk_ini`

The API spec for binary ini files in `ltk_ini`: the wire format, the key hash, the types, the
conversions, and the rules of the reader and the writer.

This document states the current behavior of `ltk_ini`. If the code and a section of this
document disagree, the section is wrong and must be edited. ADR-0024 to ADR-0027 record the
alternatives of each decision. The rules table in [section 12](#s12) cites them.

## <a id="s1"></a>1. Summary

An inibin file is the compiled form of an ini text file. The ini compiler of the game hashes each
pair of section name and property name into a 32-bit key. It selects a storage type for each
value and groups the values by storage type. The file contains the keys and the values. The file
does not contain the names.

The client loader appends `bin` to the path of the ini text file and reads the resulting path.
`.ini` becomes `.inibin` (game data), `.troy` becomes `.troybin` (particle systems) and `.cfg`
becomes `.cfgbin` (configuration). The three extensions have the same format. The property bin
format of `ltk_meta` replaced the format for most game data. The 16.20 client ships 1,589
`.inibin` and `.troybin` files ([appendix B](#appendix-b)). [Appendix A](#appendix-a) lists the
client functions that read them.

`ltk_ini` reads a file into an `Inibin`, which is a map from key to value. `ltk_ini` writes an
`Inibin` as a file. The typed getters convert a value with the conversions of the client
([section 8](#s8)). A caller accesses a value by key, or by section name and property name
([section 6](#s6)).

`ltk_ini` contains no table of section names and property names. A caller that prints a file as
ini text must provide the names. This document does not specify the ini text format.

## <a id="s2"></a>2. Vocabulary

- **inibin file** - a file in the format of [section 4](#s4), with any of the three extensions.
- **client** - the game executable.
- **ini compiler** - the tool of the game that writes an inibin file from an ini text file. The
  tool is not public. Its behavior is derived from the files that it wrote
  ([appendix B](#appendix-b)).
- **section**, **property** - the two names that identify a value in the ini text: `[System]` and
  `GroupPart0=Flash`. The type `Section` is a view of the values of one section of an `Inibin`.
- **key** - the hash of a section name and a property name ([section 5](#s5)). The type is
  `ltk_hash::IniHash`.
- **storage type** - one of the 14 encodings of a value in a file. The type is `ValueKind`. The
  discriminant of a `ValueKind` is the bit index of the storage type in the `flags` field.
- **value set** - the keys and the values of one storage type in a file. A value set is a part
  of the wire format. The data model has no value set (ADR-0025).
- **U1** - a 1-bit unsigned integer, 0 or 1. Storage type 5. The format has no boolean type. A
  `U1` holds the value of an integer property or a float property that is 0 or 1. The type is
  `Value::U1(bool)`, with `true` as 1 (ADR-0027).
- **fixed-point byte** - an unsigned byte that stores a number in units of 0.1. The byte `15`
  stores `1.5`. The range is `0.0` to `25.5`.
- **string data** - the bytes at the end of a file that contain every string. Each string has a
  NUL terminator.

## <a id="s3"></a>3. Evidence

The rules of this document are derived from three sources.

- **The client.** [Appendix A](#appendix-a) lists the functions of the 16.14 client that load a
  file, hash a key and read a value. The version 2 wire format of [section 4](#s4), the hash of
  [section 5](#s5) and every conversion of [section 8](#s8) are taken from those functions.
- **The shipped files.** [Appendix B](#appendix-b) measures 7,495 files of two client builds,
  and a corpus index of 109,712 distinct files. The writer rules of [section 9](#s9), the
  constructors of [section 7](#s7) and the `U1` storage type are derived from the measurements.
- **lolpytools.** The version 1 layout of [section 4.1](#s4.1) is taken from `read_1` in
  `lolpytools/inibin2.py`. lolpytools is the only source for version 1. No measured file has
  version 1. The 16.14 client does not branch on the version byte.

Two references differ from the client on version 2. The C# LeagueToolkit reader has 13 storage
types and no `I64`. lolpytools reads the `I64` value set after the string data. The client reads
the `I64` value set before the string data.

## <a id="s4"></a>4. Wire format (normative)

All integers are little-endian. The first byte of a file is the version. This section specifies
version 2, which is the version of every shipped file. [Section 4.1](#s4.1) specifies version 1.

```text
u8    version            2
u16   string_data_len
u16   flags              bit N is set if the value set of storage type N is present
      value sets         the present value sets, in ascending order of N
u8    string_data[string_data_len]
```

A value set:

```text
u16   count
u32   keys[count]        ascending
      values             the encoding of the storage type, see the table
```

| N  | `ValueKind` | Values of a value set                                                    |
| -- | ----------- | ------------------------------------------------------------------------ |
| 0  | `I32`       | `i32[count]`                                                             |
| 1  | `F32`       | `f32[count]`                                                             |
| 2  | `Fixed`     | `u8[count]`, each a fixed-point byte                                     |
| 3  | `I16`       | `i16[count]`                                                             |
| 4  | `U8`        | `u8[count]`                                                              |
| 5  | `U1`        | `u8[ceil(count / 8)]`. Value `i` is bit `i % 8` of byte `i / 8`.         |
| 6  | `FixedVec3` | `u8[3][count]`, each a fixed-point byte                                  |
| 7  | `Vec3`      | `f32[3][count]`                                                          |
| 8  | `FixedVec2` | `u8[2][count]`, each a fixed-point byte                                  |
| 9  | `Vec2`      | `f32[2][count]`                                                          |
| 10 | `FixedVec4` | `u8[4][count]`, each a fixed-point byte                                  |
| 11 | `Vec4`      | `f32[4][count]`                                                          |
| 12 | `String`    | `u16[count]`, each the offset of a NUL-terminated string in string data  |
| 13 | `I64`       | `i64[count]`                                                             |

The string data follows the last value set. The `I64` value set is between the string offsets
and the string data.

The client reads the version byte and does not test it. The client allocates 14 value sets and
does not test flag bits 14 and 15.

The client finds a key with a binary search over the keys of a value set. The search returns the
range of equal keys. The client reads a value only if the range contains exactly one key. The
client loads a value set that has unsorted keys or a repeated key. A lookup in such a value set
can fail.

A file with no values is the 5 bytes `02 00 00 00 00`.

### <a id="s4.1"></a>4.1 Version 1

```text
u8    version            1
u8    padding[3]
u32   count
u32   string_data_len
      entries[count]     each a u32 key and the u32 offset of a string in string data
u8    string_data[string_data_len]
```

A version 1 file has no storage types. Every value is a NUL-terminated string. This layout is
the layout that lolpytools reads ([section 3](#s3)).

## <a id="s5"></a>5. The key hash

A key is the hash of the section name, the byte `*` and the property name, in that order.

```text
hash = 0
for each byte b of the input:
    if b is an ASCII uppercase letter: b = b + 32
    hash = hash * 65599 + sign_extend(b)        (32-bit wrapping)
```

The client adds each byte as a signed 8-bit integer. A byte of `0x80` or above is added as a
negative number. The client converts only `A` to `Z` to lowercase.

`ltk_hash::sdbm::hash_lower` implements the hash. `ltk_hash::sdbm::hash_lower_from` continues a
hash state over more input. `IniHash::new(section, name)` returns the key of a pair of names.
`IniHash::hash_str("section*name")` returns the same key.

`System*GroupPart1` hashes to `0x0616933a`. The troybin fixture of [section 11](#s11) stores
the name of an emitter under that key.

## <a id="s6"></a>6. Data model

### <a id="s6.1"></a>6.1 `Inibin`

`Inibin` is a map from `IniHash` to `Value`, in ascending key order. It stores one value per key.
The storage type is a property of the value. `insert` replaces the previous value of a key,
including a previous value with a different storage type.

| Method                                   | Effect                                             |
| ---------------------------------------- | -------------------------------------------------- |
| `new()`, `Default`                       | Returns an `Inibin` with no values                 |
| `from_reader(&mut R)`                    | Reads a file ([section 9](#s9))                    |
| `to_writer(&mut W)`                      | Writes a file ([section 9](#s9))                   |
| `get(key)`, `get_mut(key)`               | Returns the value of a key                         |
| `get_str(key)`                           | Returns the string of a `Value::String`            |
| `get_i32`, `get_i64`, `get_f32`, `get_bool`, `get_vec2`, `get_vec3`, `get_vec4` | Returns the value of a key, converted as the client converts it ([section 8](#s8)) |
| `contains_key(key)`                      | Returns `true` if the key has a value              |
| `insert(key, value)`                     | Sets a value. Returns the previous value.          |
| `remove(key)`                            | Removes a value. Returns the removed value.        |
| `retain(f)`, `clear()`                   | Removes several values                             |
| `len()`, `is_empty()`                    | Returns the number of values                       |
| `iter()`, `iter_mut()`, `keys()`, `values()` | Iterates in ascending key order                |
| `section(name)`, `section_mut(name)`     | Returns a view ([section 6.2](#s6.2))              |

A `key` parameter has the type `impl Into<IniHash>`. It accepts an `IniHash`, a `u32`, and a
`&str` that is hashed as one string. A `value` parameter has the type `impl Into<Value>`.

`Inibin` implements `FromIterator<(K, V)>`, `Extend<(K, V)>` and `IntoIterator` by value, by
reference and by mutable reference.

### <a id="s6.2"></a>6.2 `Section` and `SectionMut`

`Inibin::section(name)` returns a `Section`. A `Section` is a `Copy` view that accesses the
values of one section by property name. `Section` has `key(name)`, `get(name)`,
`contains_key(name)`, `get_str(name)` and the typed getters of `Inibin`.

`Inibin::section_mut(name)` returns a `SectionMut`. `SectionMut` has `key(name)`, `get(name)`,
`get_mut(name)`, `insert(name, value)` and `remove(name)`.

A view stores the hash state after the section name and the `*`. Each lookup continues the hash
over the property name. A file does not store names. A view cannot list the properties of its
section. `section` returns a view for any name, including a name that has no values in the
`Inibin`.

### <a id="s6.3"></a>6.3 `Value` and `ValueKind`

`Value` has one variant for each storage type of [section 4](#s4). The variant has the name of
the `ValueKind`. A fixed-point variant contains the stored fixed-point bytes: `Fixed(u8)`,
`FixedVec2([u8; 2])`, `FixedVec3([u8; 3])`, `FixedVec4([u8; 4])`. `U1` contains a `bool`: `true`
is 1 and `false` is 0. The float vector variants contain `glam` vectors. `String` contains a
`String`.

`Value::kind()` returns the `ValueKind`. `ValueKind::ALL` lists the storage types in file order.
`ValueKind::flag()` returns the bit of a storage type in the `flags` field.

`From` converts a Rust value to the variant of its own type: `i32`, `f32`, `i16`, `u8`, `i64`,
`Vec2`, `Vec3`, `Vec4`, `String` and `&str`. `From` converts a `bool` to a `U1`. `From` never
returns a fixed-point variant.

`Display` writes the ini text of a value. It writes an integer in decimal, and a `U1` as `1` or
`0`. It writes a float with the fewest digits that parse back to the same `f32`. It writes the
components of a vector separated by one space.

With the `serde` feature, `Inibin`, `Value`, `ValueKind` and `IniHash` implement `Serialize` and
`Deserialize`.

## <a id="s7"></a>7. Compact constructors

The ini compiler stores a number in the smallest storage type that can store it
([appendix B](#appendix-b)). It stores an integer that is 0 or 1 as a `U1`. It stores a float
that is 0.0 or 1.0 as a `U1` or as a `Fixed`. The constructors in the table select the smallest
storage type.

| Constructor            | Result                                                                       |
| ---------------------- | ---------------------------------------------------------------------------- |
| `compact_int(i64)`     | `U1` for 0 and 1. Otherwise the first of `U8`, `I16`, `I32`, `I64` that can store the value. |
| `compact_float(f32)`   | `Fixed` if a fixed-point byte is equal to the value. Otherwise `F32`.        |
| `compact_vec2`, `compact_vec3`, `compact_vec4` | The fixed-point variant if a fixed-point byte is equal to each component. Otherwise the float variant. |

A fixed-point byte `b` is equal to a float `v` if `b as f32 / 10.0 == v` and `v` does not have a
negative sign. A compact constructor returns a value that converts back to its input exactly.

## <a id="s8"></a>8. Conversions

The ini compiler stores one property with different storage types in different files. The client
getters convert between storage types. 4,906 keys of the 16.20 client have more than one storage
type across files ([appendix B](#appendix-b)). The `to_*` methods of `Value` and the typed
getters of `Inibin` and `Section` return the value that the client getter of
[appendix A](#appendix-a) returns. A method returns `None` when the client getter reports that
the value is missing. In that case the caller of the client getter uses its default value.

| Method            | `I64` `I32` `I16` `U8` | `F32` `Fixed`          | `U1`         | `String`                         | Vector storage types |
| ----------------- | ---------------------- | ---------------------- | ------------ | -------------------------------- | -------------------- |
| `to_i64`          | the number             | truncated toward zero  | 0 or 1       | `atoi`                           | `None`               |
| `to_i32`          | the low 32 bits        | truncated toward zero  | 0 or 1       | `atoi`, the low 32 bits          | `None`               |
| `to_f32`          | the number             | the number             | 0.0 or 1.0   | `atof`                           | `None`               |
| `to_bool`         | `None`                 | `None`                 | `true` for 1 | `true` if the first byte is `1`  | `None`               |
| `to_vec2` `to_vec3` `to_vec4` | `None`     | `None`                 | `None`       | `sscanf` of N floats             | The components, if the storage type has N components. Otherwise `None`. |
| `as_str`          | `None`                 | `None`                 | `None`       | the string                       | `None`               |

- A fixed-point byte converts to the byte divided by 10.
- `atoi` skips leading whitespace, reads an optional sign and the decimal digits after it, and
  ignores the rest of the string. `atoi` returns 0 if the string has no such digits.
- `atof` skips leading whitespace, reads a decimal float with an optional fraction and exponent,
  and ignores the rest of the string. `atof` also reads `inf`, `infinity` and `nan`. `atof`
  returns 0.0 if the string does not start with a float.
- `sscanf` reads N floats and skips whitespace before each float. A string that starts with
  fewer than N floats converts to `None`. `sscanf` ignores the text after float N. `1 .5 0 0`
  converts to a `Vec2`.
- `to_i64` clamps the integer of a string to the `i64` range.

## <a id="s9"></a>9. Reading and writing rules

**Reading.** `Inibin::from_reader` takes a `Read` and does not need `Seek`. It reads the header,
the value sets or entries, and `string_data_len` bytes of string data. It does not read bytes
after the string data.

- A version byte of 2 selects the layout of [section 4](#s4). A version byte of 1 selects the
  layout of [section 4.1](#s4.1). The reader fails with `Error::UnsupportedVersion` for any other
  version byte (ADR-0026).
- The reader returns each value of a version 1 file as a `Value::String` that contains the bytes
  of the file. The typed getters convert the string ([section 8](#s8)).
- The reader fails with `Error::DuplicateKey` if a key occurs twice, in one value set or in two
  value sets (ADR-0026).
- The reader ignores flag bits 14 and 15.
- The reader accepts the keys of a value set in any order.
- The reader fails with `Error::StringOffsetOutOfBounds` if a string offset is at or after the
  end of the string data. It fails with `Error::UnterminatedString` if a string has no NUL byte
  before the end of the string data. It fails with `Error::InvalidUtf8` if a string is not UTF-8.
- The reader accepts strings that share bytes. Two keys can have the same offset. An offset can
  be inside another string.
- The reader fails with `Error::Io` if the file is truncated.

**Writing.** `Inibin::to_writer` takes a `Write` and writes version 2. It writes an `Inibin` that
was read from a version 1 file as a version 2 file with only a `String` value set.

- The writer omits the flag bit and the value set of a storage type that has no value.
- The writer writes the keys of each value set in ascending order.
- The writer sets the unused bits of the last byte of a `U1` value set to 0.
- The writer writes string data of the minimum length. It stores equal strings once. It stores
  a string that is a suffix of another string inside that string. It stores the other strings in
  ascending order of the first key that has the string.
- The writer fails with `Error::TooManyValues` if more than 65,535 values have one storage type.
  It fails with `Error::StringDataTooLong` if the string data is longer than 65,535 bytes. It
  fails with `Error::NulInString` if a string contains a NUL byte.

If a shipped file is read and written without edits, the written file is equal to the shipped
file up to the string offsets. The written file is not longer than the shipped file
([appendix B](#appendix-b)). The ini compiler orders the string data by the order of the ini
text. The file does not record that order. The string offsets and the string data of the written
file can differ from the shipped file.

## <a id="s10"></a>10. Errors

`ltk_ini::Error` is `#[non_exhaustive]`. `ltk_ini::Result<T>` is the alias.

| Variant                    | Returned by | Condition                                              |
| -------------------------- | ----------- | ------------------------------------------------------ |
| `UnsupportedVersion(u8)`   | reader      | The version byte is not 1 or 2                         |
| `DuplicateKey(IniHash)`    | reader      | A key occurs more than once in the file                |
| `StringOffsetOutOfBounds`  | reader      | A string offset is not inside the string data          |
| `UnterminatedString`       | reader      | A string has no NUL byte before the end of string data |
| `InvalidUtf8`              | reader      | A string is not UTF-8                                  |
| `NulInString`              | writer      | A string contains a NUL byte                           |
| `TooManyValues`            | writer      | More than 65,535 values have one storage type          |
| `StringDataTooLong`        | writer      | The string data is longer than 65,535 bytes            |
| `Io(std::io::Error)`       | both        | The reader or the writer failed, or the file is truncated |

## <a id="s11"></a>11. Testing

- **Unit tests** in `src/` build files byte by byte. They cover each rule of [section 9](#s9),
  each error of [section 10](#s10), each cell of the table in [section 8](#s8) and each
  constructor of [section 7](#s7).
- **Version 1 tests** build their files from the layout of [section 4.1](#s4.1). No shipped file
  has version 1.
- **Fixture tests** in `tests/fixtures.rs` read two shipped files of the 16.20 client.
  `global_item_travelsizeelixirofiron_buf.troybin` has values of every storage type except `I64`.
  It stores `p-life` as `F32` for one emitter and as `Fixed` for another emitter.
  `aatroxbotsummonerspellsclassic.inibin` has repeated strings. The writer writes it byte for
  byte.
- **The corpus test** in `tests/corpus.rs` is ignored by default. It reads and writes every
  `.inibin`, `.troybin` and `.cfgbin` file in the directory that `LTK_INI_DIR` names. The test
  fails if a file does not read to its last byte, if the written file is longer than the shipped
  file, or if the written file does not read back with the same values. The test also fails if
  the written file differs from the shipped file before the string offsets.

## <a id="s12"></a>12. Rules

| ID  | Rule | Instead of | Why | Spec |
| --- | ---- | ---------- | --- | ---- |
| I1  | The crate is `ltk_ini` and the file type is `Inibin`. | A module of `ltk_meta`. A crate that has one extension in its name. | The format shares no code with property bins. The three extensions have one format. | [section 1](#s1); ADR-0024 |
| I2  | `Inibin` is one map from key to value. | One collection per storage type, which is the layout of the file and of the C# reader. | Every shipped file stores one value per key. | [section 6.1](#s6.1); ADR-0025 |
| I3  | The reader rejects a version byte other than 1 or 2. The reader rejects a repeated key. | Loading both, as the client does. | The map stores one value per key. The client reads no value for a repeated key of one value set. | [section 9](#s9); ADR-0026 |
| I4  | The reader and the writer place the `I64` value set between the string offsets and the string data. | After the string data (lolpytools). No `I64` value set (C# LeagueToolkit). | The client loader reads the value sets in bit order and reads the string data last. | [section 4](#s4) |
| I5  | The hash converts `A` to `Z` to lowercase and adds each byte sign-extended. | Unicode lowercasing. Unsigned bytes. | The client hash does both. | [section 5](#s5) |
| I6  | `U8` is unsigned. | A signed `I8`. | The ini compiler stores 128 to 255 as `U8` and stores -1 as `I16`. | [section 4](#s4); [appendix B](#appendix-b) |
| I7  | The typed getters and the `to_*` methods convert as the client getters do. | Returning a value only for its own storage type. | The ini compiler selects the storage type per file. A caller does not know the storage type. | [section 8](#s8) |
| I8  | `to_i32` and `to_f32` convert a string that has no leading number to 0. | `None`. | The client getter reports a value and returns 0. | [section 8](#s8) |
| I9  | `to_bool` returns `None` for an integer other than a `U1`. | Converting a nonzero integer to `true`. | The client getter reads only the `U1` value set and the `String` value set. | [section 8](#s8) |
| I10 | `Display` writes a float with the fewest digits that parse back to the same `f32`. | The `%g` format of the client string getter. | `%g` writes 6 significant digits and discards the other digits. | [section 6.3](#s6.3) |
| I11 | `From` keeps the type of its input. The `compact_*` constructors select the smallest storage type. | One conversion that always selects the smallest storage type. | A caller that replaces a value can keep the storage type of the value. | [section 6.3](#s6.3), [section 7](#s7) |
| I12 | The writer orders the string data by key and stores a suffix inside the longer string. | The order of the ini compiler, which is the order of the ini text. | The file does not record the order of the ini text. String data of the minimum length fits in 65,535 bytes for every set of strings that any valid file contains. | [section 9](#s9) |
| I13 | The writer sorts the keys of each value set. | Insertion order. | The client finds a key with a binary search. | [section 4](#s4) |
| I14 | A string is a UTF-8 `String`. | A byte string. | Every string of the measured files is ASCII. | [section 9](#s9); [appendix B](#appendix-b) |
| I15 | The crate contains no table of section names and property names. | A built-in dictionary. | A name table is data that changes with each game patch. | [section 1](#s1) |
| I16 | The reader reads version 1 with the layout of lolpytools. | The layout of the C# reader. No version 1 reader. | The C# reader reads offsets and no keys. lolpytools reads a key and an offset for each entry. | [section 4.1](#s4.1); ADR-0026 |
| I17 | A version 1 value is a `Value::String`. The writer writes only version 2. | Converting each string to a typed value in the reader, as lolpytools does. A version 1 writer. | The string is the stored value, and the typed getters convert it. The 16.14 client reads only the version 2 layout. | [section 9](#s9) |
| I18 | Storage type 5 is `U1`, a 1-bit unsigned integer. `Value::U1` contains a `bool`. | `Bool`. `U1(u8)`. | The ini compiler stores integers and floats that are 0 or 1 in storage type 5. The format has no boolean type. | [section 2](#s2), [section 4](#s4); ADR-0027 |

## <a id="appendix-a"></a>Appendix A. Client functions

League of Legends 16.14, macOS x86_64, build 16.14.7949266. Each address is a virtual address
in that executable.

| Address       | Function                                                                               |
| ------------- | -------------------------------------------------------------------------------------- |
| `0x1016FBAC0` | Config object constructor. Calls the loader with the path, then with the default path.  |
| `0x1016FB3F0` | Loader. Appends `bin` to the path. Reads the header, 14 value sets and the string data. |
| `0x101700F30` | Reads the `U1` value set: `ceil(count / 8)` bytes.                                      |
| `0x101703350` | Reads the `String` value set: keys and `u16` offsets.                                   |
| `0x1017036F0` | Reads the `I64` value set: keys and 8-byte values.                                      |
| `0x1016FA740` | String getter by names. Contains the key hash inline, with `movsx` on each byte.        |
| `0x1016FCD00` | Integer lookup. Searches `I64`, `I32`, `I16`, `U8` in that order.                       |
| `0x1016FCA60` | Float lookup. Searches `F32`, then `Fixed`. Divides a `Fixed` byte by 10.               |
| `0x1016FD700` | Integer getter. Tries the integer lookup, the float lookup, `U1`, then `atoi` of `String`. |
| `0x1016FC6B0` | Float getter. Tries the float lookup, the integer lookup, `U1`, then `atof` of `String`. |
| `0x1016FDAA0` | Boolean getter. Tries `U1`, then compares the first byte of `String` with `1`.          |
| `0x1016FD200` | 3-float getter. Tries `Vec3`, `FixedVec3` divided by 10, then `sscanf("%f %f %f")` of `String`. |
| `0x1016FE2E0` | String getter by key. Tries `String`, an integer as `%lli`, a float as `%g`, `U1` as `1` or `0`, then the vectors as `%g` lists. |

## <a id="appendix-b"></a>Appendix B. Corpus measurements

Measured on 2026-10-12.

**Client 16.20** (build 16.20.8248524): every `.inibin`, `.troybin` and `.cfgbin` file in the 393
WADs of an installed client. The WADs contain 400 `.inibin` files and 1,189 `.troybin` files with
136,204 values. **Client 1.0.0.126**: every such file in the unpacked `DATA` and `LEVELS`
directories. The directories contain 3,140 `.inibin` files and 2,766 `.troybin` files with 583,223
values. The two clients ship no `.cfgbin` file.

| Measurement                                              | 16.20        | 1.0.0.126    |
| -------------------------------------------------------- | ------------ | ------------ |
| Files                                                    | 1,589        | 5,906        |
| Files with version byte 2                                | 1,589        | 5,906        |
| Files with flag bit 13, 14 or 15                         | 0            | 0            |
| Files with unsorted keys in a value set                  | 0            | 0            |
| Files with a key in two value sets or twice in one value set | 0        | 0            |
| Files with bytes after the string data                   | 0            | 0            |
| Files with string data bytes that no offset reaches      | 0            | 0            |
| Files with a nonzero unused bit in the `U1` value set    | 0            | 0            |
| Files with a string that is not ASCII                    | 0            | 0            |
| Files with no values (5 bytes)                           | 4            | 4            |
| Files that repeat a string offset                        | 1,409        | 4,699        |
| Files with an offset inside another string               | 0            | 2,877        |
| Files that store one string twice                        | 0            | 3,056        |
| Keys with more than one storage type across files        | 4,906 of 54,080 | 6,911 of 89,935 |
| `I32` values that fit in an `I16`                        | 0 of 29      | 0 of 1,142   |
| `I16` values that fit in a `U8`                          | 0 of 1,187   | 0 of 7,945   |
| `U8` values equal to 0 or 1                              | 0 of 7,388   | 0 of 30,868  |
| Range of `U8` values                                     | 2 to 252     | 2 to 255     |
| Smallest `I16` value                                     | -10,011      | -1,495       |
| `F32` values equal to a fixed-point byte                 | 0 of 3,679   | 0 of 8,820   |
| Float vectors with every component equal to a fixed-point byte | 0 of 13,016 | 0 of 45,507 |
| Files that read, write and read back with equal values   | 1,589        | 5,906        |
| Written files equal to the shipped file up to the string offsets | 1,589 | 5,906       |
| Written files equal to the shipped file                  | 310          | 1,232        |
| Written files shorter than the shipped file              | 409          | 4,245        |
| Written files longer than the shipped file               | 0            | 0            |

Measured on 2026-10-25. **Corpus index**: the `inibin-parse` index of the RADS archive, local
client installs and every live build of the lol-census database up to 9.x. The index holds
106,096 distinct version 2 files and 3,616 distinct version 1 files with 16,220,163 values.

| Measurement                                                          | Count            |
| -------------------------------------------------------------------- | ---------------- |
| `U1` values                                                          | 2,436,557        |
| Keys with a `U1` value in at least one file                          | 130,430          |
| ... with a `U8` value from 2 to 255 in another file                  | 8,630            |
| ... with an `I16` value in another file                              | 1,464            |
| ... with an `I32` value in another file                              | 30               |
| ... with a `Fixed` value in another file                             | 6,722            |
| ... with an `F32` value in another file                              | 2,143            |
| `I32`, `I16`, `U8` and `F32` values equal to 0 or 1                  | 0                |
| `Fixed` values equal to 0.0 or 1.0                                   | 40,888           |
| `String` values `true`, `false`, `yes`, `no`, `on` or `off`, any case | 32,408          |
