# ADR-0027: U1 storage type

- **Status:** Proposed
- **Date:** 2026-10-25
- **Crates:** `ltk_ini`
- **Related:** ADR-0025, `docs/design/inibin.md` [section 4](../design/inibin.md#s4),
  [section 6.3](../design/inibin.md#s6.3), [section 7](../design/inibin.md#s7) and
  [section 8](../design/inibin.md#s8)

## Context and problem statement

Storage type 5 packs one bit per value (`docs/design/inibin.md` [section 4](../design/inibin.md#s4)).
`ltk_ini` 0.1.0 names it `ValueKind::Bool` and its value `Value::Bool(bool)`. The documentation of
`Value::compact_int` states that the ini compiler stores 0 and 1 as booleans.

A corpus index of 106,096 distinct version 2 files and 3,616 distinct version 1 files measures
the storage type 5 values. The files are the RADS archive, local client installs and every live
build of the lol-census database up to 9.x.

- 130,430 keys have a storage type 5 value in at least one file.
- 8,630 of those keys are `U8` in another file, with values from 2 to 255:
  `Data*AttackRank`, `Data*ChampionId`, `Data*ItemType`. 1,464 are `I16` and 30 are `I32` in
  another file.
- 6,722 of those keys are `Fixed` in another file and 2,143 are `F32`:
  `Data*AttackSpeedPerLevel`, `MeshSkin*SkinScale`, `Data*HPRegenPerLevel`.
- No `I32`, `I16`, `U8` or `F32` value is 0 or 1. 40,888 `Fixed` values are 0.0 or 1.0.
- 32,408 values are the strings `true`, `false`, `yes`, `no`, `on` or `off`, in any case. Each
  of them is a `String` value.

The ini compiler stores a number that is 0 or 1 in storage type 5, for integer properties and
for float properties. It has no boolean type. The client integer getter and float getter read
storage type 5 as 0 and 1. The client boolean getter reads storage type 5 and the first byte of a
string ([appendix A](../design/inibin.md#appendix-a)).

## Decision drivers

- A variant name states what the value is. A caller that matches `Value::Bool` on
  `Data*AttackRank` reads a flag where the file holds a rank.
- `ltk_ini` 0.1.0 is published. Every option except the first is a breaking change.
- A variant holds only values that the storage type can store.

## Considered options

1. **Keep `Bool`, correct the documentation.**
2. **`U1(bool)`**: rename the storage type and the variant to `U1`, keep the `bool` payload.
3. **`U1(u8)`**: rename, and hold the number as a `u8` that is 0 or 1.

## Decision

**Option 2. Storage type 5 is `ValueKind::U1`, and its value is `Value::U1(bool)`. `true` is 1
and `false` is 0.**

`compact_int` returns `U1` for 0 and 1. `From<bool>` returns `U1`. The typed getters convert a
`U1` as the client getters do, including `to_bool`. `docs/design/inibin.md`
[section 4](../design/inibin.md#s4), [section 6.3](../design/inibin.md#s6.3) and
[section 8](../design/inibin.md#s8) state the rules.

## Consequences

- **Positive:** the variant name states that the value is a 1-bit unsigned integer, next to
  `U8`, `I16`, `I32` and `I64`. Every `U1` value is a value the file can store, and the writer has
  no new error.
- **Negative:** a breaking change to a published crate: `ValueKind::Bool`, `Value::Bool` and the
  serde form `{"Bool": true}` are removed. The payload type is still `bool`. A caller reads
  `U1(true)` as the number 1 only with the documentation of the variant.
- **Revisit when:** a client build stores a value other than 0 or 1 in storage type 5.

## Pros and cons of the options

### Option 1: keep `Bool`, correct the documentation

- Good: no breaking change. The serde form stays the same.
- Bad: the name contradicts the corrected documentation. A caller that reads `Bool` on a rank, a
  champion ID or a float property treats a number as a flag.

### Option 3: `U1(u8)`

- Good: the payload type is a number, like the payload of `U8`. `U1(1)` reads as 1 without
  documentation.
- Bad: the type holds 254 values that the storage type cannot store. The writer needs an error
  for a `U1` above 1, or it truncates the value and writes a different number than the caller
  set. Each conversion tests the payload against 0.
