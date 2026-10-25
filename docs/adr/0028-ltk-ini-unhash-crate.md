# ADR-0028: `ltk_ini_unhash` crate

- **Status:** Accepted
- **Date:** 2026-10-25
- **Crates:** `ltk_ini_unhash`, `ltk_ini`
- **Related:** ADR-0024, `docs/design/inibin.md` [section 1](../design/inibin.md#s1) and rule I15,
  `docs/design/inibin-unhash.md`

## Context and problem statement

An inibin file stores the 32-bit SDBM hash of each section name and property name, and no names
(`docs/design/inibin.md` [section 5](../design/inibin.md#s5)). `ltk_ini` contains no table of names
(rule I15). A tool that prints a file as ini text, or edits a property by name across files, needs
the names.

The other hashed names of the game are resolved from tables of hash to name. A WAD path is the
64-bit XXHash64 of the path. A bin name is the FNV-1a hash of a name from a fixed set of classes
and properties. An inibin key differs from both:

- Many section names are values of the same file. A `.troybin` file names its emitter sections in
  `[System] GroupPart1`, `GroupPart2`, and so on. Particle types, map objects, fields and groups
  are sections named by values too. A value is any text: the emitter of one shipped file is the
  section `123451`.
- Numbered lists are open-ended. The client reads `GroupPart` up to the first missing part, and a
  file holds parts after a gap.
- The key is 32 bits, and SDBM collides far more often than an ideal 32-bit hash. One hash names
  unrelated keys of different file families. The loader that reads a file tells them apart.
- A loader reads a key with one getter. A file holds keys that the loader never reads: typos,
  parts after a gap, and lines that the ini compiler stored with a comment prefix (`'GroupPart3`).

The `inibin-parse` research tool resolves 96.0% of the 16,220,163 values of a corpus index of
109,712 distinct files. It groups names into 66 classes of files, each read by a recipe that walks
a file the way the client loader of that family walks it.

## Decision drivers

- A section named by a value has a name that no list holds in advance.
- A recipe reads a file the way the client loader reads it. A reader of a recipe reads the loader.
- The name of a key carries the type its getter reads, and whether the client reads the key.
- A name table changes with each patch. `ltk_ini` stays free of it.
- A consumer chooses whether to name keys that the client never reads.

## Considered options

1. **A new crate `ltk_ini_unhash` of recipes in Rust.** Each recipe is a function over a reader
   of one file. Declarative macros keep the name lists dense.
2. **Tables from hash to name**, global or per class, built by running the recipes over a corpus,
   as for WAD paths and bin names.
3. **A module of `ltk_ini`** with the recipes of option 1.

## Decision

**Option 1. `ltk_ini_unhash` depends on `ltk_ini`. A recipe reads a file through a reader that
names each key it reads. The recipes of the 66 research classes are modules of 8 broad classes.
The caller names the class of a file, or tries every class.**

The crate names every key that a recipe reads by default, including keys that the client never
reads and keys with a comment prefix. Options exclude either group. The path patterns of the
research classes are comments, not code. `docs/design/inibin-unhash.md` specifies the surface.

## Consequences

- **Positive:** a recipe names a section that a value of the file names, in any file, including
  a file that no corpus holds. A name carries its declared type and whether the client reads the
  key. The compiler checks every recipe. `ltk_ini` gains no dependency and no data.
- **Negative:** naming a file runs its recipe, not one map access per key. A recipe change is a
  code change and a release of the crate. The ported recipes and the `inibin-parse` recipes can
  drift apart. The crate does not classify a file by its path. A caller that has only a path
  selects the class itself, or tries every class and takes the one that names the most keys.
- **Revisit when:** recipes change faster than the crate is released, or a consumer needs the class
  of a file from its path.

## Pros and cons of the options

### Option 2: tables from hash to name

- Good: a lookup is one map access, and the crate holds data, not logic. The workspace resolves
  WAD paths and bin names the same way. A table can store every `section*name` pair that a recipe
  produces, unrolled.
- Bad: a table holds only the section names that the corpus has. An emitter of a new file, or of a
  file outside the corpus, has no entry. A global table maps one hash to several unrelated names;
  a table per class needs the class of the file as well. A table loses what a walk knows: the type
  of the getter, and whether the client reads the key.

### Option 3: a module of `ltk_ini`

- Good: the workspace gets no new crate.
- Bad: every `ltk_ini` consumer compiles the recipes of 66 classes. Rule I15 of
  `docs/design/inibin.md` keeps the tables out of `ltk_ini`.
