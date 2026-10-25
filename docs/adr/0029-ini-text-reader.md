# ADR-0029: Ini text reader

- **Status:** Proposed
- **Date:** 2026-10-25
- **Crates:** `ltk_ini`, `ltk_ini_unhash`
- **Related:** ADR-0026, ADR-0028, `docs/design/inibin.md` [section 9.1](../design/inibin.md#s9.1)
  and [appendix B](../design/inibin.md#appendix-b)

## Context and problem statement

An inibin file is the compiled form of an ini text file. `ltk_ini_unhash::to_ini` writes a named
file as ini text. A tool that edits a file as text needs the inverse: ini text to an `Inibin`.

The RADS archive holds 4,452 ini text files next to the binary file of the same path in the same
release. 477 distinct pairs measure the rules of the ini compiler
([appendix B](../design/inibin.md#appendix-b)):

- The compiler cuts a line at its first `;`. 316 of 316 values with an inline comment are stored
  without it: `30 ; Burn total damage to monsters` is stored as `30`.
- The compiler trims keys and values. 912 of 912 values with surrounding whitespace are stored
  trimmed.
- A line without `=` is never a key (444 lines).
- A typed value has the storage type of the `compact_*` constructors, except a float `0.0` that
  is a `U1`. Other values with the same text are a `String`: `Item1=3006` is an `I16` and
  `Item3=3020` of the same section is a `String`. The same keys are a `String` in every release.
  The text does not determine which values are a `String`.
- The compiler of old builds stores a line that starts with `'`, `*`, `#`, `//` or `` ` `` as a
  property with that prefix in its name. Shipped binaries hold such keys.

## Decision drivers

- Reading the text of a shipped file names the same keys as the compiled file.
- The text does not determine which values the compiler keeps as a `String`.
- A tool that edits text chooses whether prefixed lines are properties or comments.
- `to_ini` output reads back to the same keys and values.

## Considered options

1. **`Inibin::from_ini` in `ltk_ini`, every value a `Value::String`.** Options select whether the
   prefixed lines are comments.
2. **The same reader, with the `compact_*` constructors selecting a storage type** for each value
   that parses as numbers.
3. **The reader in `ltk_ini_unhash`**, next to `to_ini`.

## Decision

**Option 1. `Inibin::from_ini(text, IniTextOptions)` reads ini text into an `Inibin`. Every
value is a `Value::String`. A `;` starts a comment. `IniTextOptions::additional_comment_tokens`
makes a line that starts with a prefix a comment. By default such a line is a property.**

`docs/design/inibin.md` [section 9.1](../design/inibin.md#s9.1) specifies the rules. The reader
returns no error.

## Consequences

- **Positive:** the reader names the keys that the compiler names, and stores the text of each
  value. The typed getters convert a string as the client does (`docs/design/inibin.md`
  [section 8](../design/inibin.md#s8)). `ltk_ini` needs no names to read text.
- **Negative:** a file written from text stores every value as a string. It is larger than the
  compiled file, and its storage types differ from the compiled file. A repeated key takes the
  value of its last line, a rule that no measured file tests.
- **Revisit when:** the property types of the compiler are known, or a consumer needs the
  compiled storage types.

## Pros and cons of the options

### Option 2: compact storage types

- Good: a file written from text is close in size to the compiled file. The selected type is the
  type of the compiler for 24,838 of 37,374 numeric values of the measured pairs. It differs in
  11,955 values that the compiler keeps as a `String`, and in 581 others.
- Bad: a string property with a numeric value, such as the emitter name `123451`, becomes a
  number. The string getter of the client formats a number with `%g`: `0.0000 0.0000 0.0000`
  reads back as `0 0 0`.

### Option 3: the reader in `ltk_ini_unhash`

- Good: the text format lives in one crate.
- Bad: reading text needs no names. A consumer that edits text compiles the recipes of every
  class.
