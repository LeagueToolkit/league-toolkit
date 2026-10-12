# ADR-0025: Flat inibin value map

- **Status:** Proposed
- **Date:** 2026-10-12
- **Crates:** `ltk_ini`
- **Related:** #119, #121, #122, ADR-0026, `docs/design/inibin.md`
  [section 6.1](../design/inibin.md#s6.1)

## Context and problem statement

An inibin file groups its values by storage type into 14 value sets. Each value set is a list of
keys and a list of values (`docs/design/inibin.md` [section 4](../design/inibin.md#s4)). The C#
LeagueToolkit has the same layout: `InibinFile.Sets` is a dictionary from storage type to
`InibinSet`. Issue #119 asks for that layout, and #121 and #122 implement it.

The ini compiler of the game selects the storage type of each value per file. One property is
`F32` in one file and `Fixed` in another file. 4,906 of the 54,080 keys of the 16.20 client have
more than one storage type across files. A caller that looks up a value does not know its storage
type. The client getters search several value sets for one key
([appendix A](../design/inibin.md#appendix-a)).

No shipped file stores a key in two value sets
([appendix B](../design/inibin.md#appendix-b)).

## Decision drivers

- A lookup by key is the main operation. The only input of a lookup is the key.
- A key has one value, including when an edit changes the storage type of the value.
- The value sets are a layout of the file. The writer can build them from any collection of
  values.

## Considered options

1. **One map from key to value.** The storage type is the variant of the value.
2. **One public collection per storage type**, which is the layout of the C# reader.
3. **Private value sets with a key-based API** that searches them.

## Decision

**Option 1. `Inibin` is one ordered map from `IniHash` to `Value`. `Value::kind()` returns the
storage type. The writer groups the values into value sets.**

`insert` replaces the previous value of a key, including a previous value with a different
storage type. The reader rejects a file that stores a key twice (ADR-0026).
`docs/design/inibin.md` [section 6.1](../design/inibin.md#s6.1) specifies the surface.

## Consequences

- **Positive:** `get`, `insert` and `remove` are one map operation each. A key cannot have two
  values. Iteration is in key order, which is also the order of the keys of each value set in a
  file.
- **Negative:** an `Inibin` cannot represent a file that stores one key in two value sets. A
  caller that lists the values of one storage type must filter all values. The writer groups all
  values on each write.
- **Revisit when:** a shipped file stores one key in two value sets and the client reads both
  values.

## Pros and cons of the options

### Option 2: public collections per storage type

- Good: the data model has the layout of the file and of the C# reader.
- Bad: a lookup needs a storage type that the caller does not know, or it searches 14
  collections. An insert with a new storage type leaves the previous value in its collection
  unless every caller removes it. Each collection must validate the variant of every inserted
  value.

### Option 3: private value sets with a key-based API

- Good: the public surface is the same as in option 1. The writer does not need to group the
  values.
- Bad: each lookup searches up to 14 collections and each insert removes from 13 collections.
  The writer of option 1 groups the values in one pass over the map.
