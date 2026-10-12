# ADR-0026: Strict inibin reader

- **Status:** Proposed
- **Date:** 2026-10-12
- **Crates:** `ltk_ini`
- **Related:** ADR-0025, `docs/design/inibin.md` [section 4.1](../design/inibin.md#s4.1),
  [section 9](../design/inibin.md#s9) and [section 10](../design/inibin.md#s10)

## Context and problem statement

The client loader of an inibin file does not validate the file
(`docs/design/inibin.md` [appendix A](../design/inibin.md#appendix-a)).

- The loader reads the version byte and does not test it.
- The loader accepts a value set that has a repeated key. A lookup of that key returns no value:
  the binary search returns a range of two keys, and the client reads a value only from a range
  of one key.
- The loader accepts one key in two value sets. Each getter returns the value of the first value
  set in its own search order.
- A getter reads a string from its offset to the next NUL byte and does not check the length of
  the string data.

Each of the 7,495 measured files has version byte 2, unique keys, and terminated ASCII strings
inside the string data ([appendix B](../design/inibin.md#appendix-b)).

Two older tools read a version 1 layout. A version 1 file is a list of string values and has no
storage types. The two tools read different layouts. lolpytools reads a 32-bit count, a 32-bit
string data length, and a key and a 32-bit offset for each entry. The C# LeagueToolkit reads
16-bit offsets and no keys, and then indexes an empty key list. No measured file has version 1.
The 16.14 client reads every file with the version 2 layout.

## Decision drivers

- `Inibin` stores one value per key (ADR-0025).
- If the reader accepts a file, the written file has the same values.
- The C# LeagueToolkit accepts version 1, and this crate replaces the C# reader.
- A reference that specifies a complete version 1 layout is sufficient to read version 1.

## Considered options

1. **Accept every file that the client loads.** Ignore the version byte. Keep the last value of
   a repeated key. Read a string to the end of the string data if it has no terminator.
2. **Reject a file that `Inibin` cannot represent, and reject version 1.** Fail on a version
   byte other than 2, on a repeated key, on a string outside the string data, on a string with no
   terminator and on a string that is not UTF-8.
3. **Reject a file that `Inibin` cannot represent, and read version 1 with the layout of
   lolpytools.**

## Decision

**Option 3. The reader reads version 2 and version 1. The reader fails on any other version
byte, on a key that occurs more than once in the file, and on a string that is outside the string
data, has no terminator or is not UTF-8.**

The version 1 layout is the layout that lolpytools reads. A version 1 value is a
`Value::String`. The writer writes only version 2. The errors are `UnsupportedVersion`,
`DuplicateKey`, `StringOffsetOutOfBounds`, `UnterminatedString` and `InvalidUtf8`. The reader
ignores flag bits 14 and 15 and accepts unsorted keys, as the client does.
`docs/design/inibin.md` [section 4.1](../design/inibin.md#s4.1) and
[section 9](../design/inibin.md#s9) specify the rules.

## Consequences

- **Positive:** an `Inibin` contains every value of a file that the reader accepts. A repeated
  key fails with an error that contains the key. The reader accepts a version 1 file that has the
  lolpytools layout, and the writer converts it to the version that the client reads.
- **Negative:** the crate rejects some files that the client loads. The client loads a file that
  has a repeated key and returns no value for that key. This crate cannot read that file to
  repair it. The crate cannot read a file that has a string in a legacy code page. The version 1
  reader is derived from one reference, and no file verifies it. If a version 1 file has a
  different layout, the reader returns incorrect values or fails with an error that does not
  identify the layout as the cause.
- **Revisit when:** a shipped file fails to read, or a version 1 file is found. Change the rule
  to match the file.

## Pros and cons of the options

### Option 1: accept every file that the client loads

- Good: the reader accepts every file that the client loads.
- Bad: the reader discards one value of a repeated key and does not report it. The written file
  differs from the file that was read. The reader parses a version 1 file with the version 2
  layout and returns incorrect values.

### Option 2: reject version 1

- Good: every rule of the reader is verified against the client or against a shipped file.
- Bad: the crate rejects version 1 files, which the C# reader accepts. The crate cannot read a
  version 1 file that has the lolpytools layout.
