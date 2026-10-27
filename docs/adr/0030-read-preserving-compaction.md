# ADR-0030: Read-preserving compaction

- **Status:** Proposed
- **Date:** 2026-10-25
- **Crates:** `ltk_ini`
- **Related:** ADR-0027, ADR-0029, `docs/design/inibin.md` [section 7.1](../design/inibin.md#s7.1)
  and [appendix B](../design/inibin.md#appendix-b)

## Context and problem statement

`Inibin::from_ini` stores every value as a `Value::String` (ADR-0029). The ini compiler stores
most numbers in a numeric storage type, and some as a string. An edit that sets a value with
`From` keeps the storage type of its input. A pass that moves values into smaller storage types
needs a rule for which conversions it makes.

The getters of the client read a storage type in fixed ways (`docs/design/inibin.md`
[section 8](../design/inibin.md#s8)). The boolean getter reads a `U1` and a string: a string is
`true` if it starts with `1`. It reads no other storage type. The string `10` has a boolean read
of `true`, and `U8(10)` has none. A `U1` has a boolean read, and a `U8` 1 has none.

In 20,000 shipped files, a conversion of each string whose number has the same text changes the
boolean read of 38,496 values. On 477 ini text pairs, it makes the written files larger than the
strings: 435,733 bytes against 383,408 ([appendix B](../design/inibin.md#appendix-b)). The
string data stores each text once, and repeated vectors cost 2 bytes per key as strings.

## Decision drivers

- A compacted value reads the same through every getter. The client cannot tell the two apart.
- A compacted file is not larger than the file it comes from.

## Considered options

1. **Read-preserving.** A conversion is made only if every `to_*` read and the text are the same.
2. **Text-preserving.** A string becomes the compact value of its number if the text is the same.
3. **Like the compiler.** A string becomes the compact value of each number it parses as.

## Decision

**Option 1. `Value::compact` converts a value only into a storage type with the same `to_*`
reads and the same text. `Inibin::compact` compacts each value.**

The strings `0` and `1` become a `U1`. An integer becomes the smallest integer storage type other
than `U1`. A float and a vector become the compact constructors of their numbers.
`docs/design/inibin.md` [section 7.1](../design/inibin.md#s7.1) specifies the rule.

## Consequences

- **Positive:** a compacted file reads the same as the file it comes from, for the client and for
  `ltk_ini`. On the measured text pairs it is smaller than the strings: 366,526 bytes.
- **Negative:** a numeric string other than `0` and `1` stays a string. The storage types of a
  compacted text file match the compiler for 23,378 of 41,749 values. An integer 0 or 1 of a
  numeric storage type stays a `U8`, one byte larger than a `U1`.
- **Revisit when:** a consumer needs the storage types of the compiler, or the boolean getter of
  the client is shown to read other storage types.

## Pros and cons of the options

### Option 2: text-preserving

- Good: numeric strings such as `200` become numbers, closer to the compiler.
- Bad: the boolean read of a numeric string changes from a value to none. On the measured pairs
  the written files grow by 14%.

### Option 3: like the compiler

- Good: the closest storage types to the compiler.
- Bad: text that is not the text of its number changes: `0.0000 0.0000 0.0000` reads back as
  `0 0 0`, and `007` as `7`. Every boolean read of a numeric string changes.
