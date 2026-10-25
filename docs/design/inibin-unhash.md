# Inibin names in `ltk_ini_unhash`

The API spec for naming the keys of inibin files in `ltk_ini_unhash`: the classes, the reads of a
recipe, the ranking of names, the options, and the tests.

This document states the current behavior of `ltk_ini_unhash`. If the code and a section of this
document disagree, the section is wrong and must be edited. ADR-0028 records the alternatives of
the crate. The rules table in [section 11](#s11) cites it.

## <a id="s1"></a>1. Summary

An inibin file stores the 32-bit hash of each section name and property name, and no names
(`inibin.md` [section 5](inibin.md#s5)). `ltk_ini_unhash` names the keys of an `Inibin`.

A recipe reads a file the way the client loader of a family of files reads it. Each read of a key
that the file has assigns a name to the key. Some section names are values of the file: a
`.troybin` file names its emitter sections in `[System] GroupPart1`, `GroupPart2`, and so on. A
recipe reads such a value and reads the section that it names.

A caller names a file with the recipe of one class ([section 5](#s5)), or with the recipe of every
class ([section 5.3](#s5.3)). The result maps each named key to its best names
([section 7](#s7)). A name records its declared type and whether the client reads the key.
`to_ini` writes a named file as ini text ([section 5.4](#s5.4)).

## <a id="s2"></a>2. Vocabulary

- **key** - the SDBM hash of `section*name`, an `ltk_hash::IniHash` (`inibin.md`
  [section 5](inibin.md#s5)).
- **section state** - the SDBM hash of `section*`. The key of a property is the hash of its name,
  continued from the section state.
- **class** - a family of files that one recipe names. The type is `Class`.
- **recipe** - the reads of one class, as Rust functions written in the `recipe!` macro
  ([section 9](#s9)).
- **read** - one lookup of `section*name` by a recipe, with a declared type or no type.
- **declared type** - the type of the client getter of a property. The type is `Ty`. A declared
  type is not a storage type: a file stores an `Int` property as an `I32`, an `I16`, a `U8` or a
  `U1` (`inibin.md` [section 2](inibin.md#s2)).
- **claim** - a name that a read assigns to a key. A key has a claim for each distinct name that
  a read of the key produces.
- **client read** - a read on a path that the client takes.
- **stale read** - a read on a path that the client never takes: a typo, an item past a count, a
  part after a gap, the keys of the other reader of an emitter.
- **comment prefix** - one of `'`, `*`, `#`, `//` and `` ` ``. The ini compiler of old builds
  stores a line that starts with a comment prefix as a property: `'GroupPart3`. The client never
  reads such a property. The constant is `COMMENT_PREFIXES`.
- **name** - the section name and property name of a key, with its declared type and its stale
  flag. The type is `Name`. A name is **stale** if no client read claims it, or if it has a comment
  prefix.
- **history** - the string values that other versions of a file hold, by key. The type is
  `History`.
- **research class** - one of the 66 classes of the `inibin-parse` research tool. A research
  class is a module of a recipe file, and its path patterns are comments of the module.

## <a id="s3"></a>3. Evidence

The rules of this document are derived from three sources.

- **The client.** The loaders of the 1.0.0.126 client and the 4.17 macOS client fix the reads of
  the recipes that cite them. `CharacterData::LoadIdleParticles` and
  `CharacterData::LoadDefaultAnimations` read a count and then items 1 to the count
  ([section 6.5](#s6.5)).
- **The research tool.** `inibin-parse` names 96.0% of the 16,220,163 values of a corpus index of
  109,712 distinct files. Its recipes are the source of the recipes of this crate.
- **The corpus.** The RADS archive of shipped clients. [Appendix A](#appendix-a) holds the
  measurements against it.

## <a id="s4"></a>4. Data model

### <a id="s4.1"></a>4.1 `Class`

```rust
#[non_exhaustive]
pub enum Class { Character, Spell, Globals, Ui, Particles, Troybin, Level, Scene }

impl Class {
    pub const ALL: &'static [Class];
}
```

`Class::ALL` lists every class in the order of the enum. [Section 10](#s10) lists the files of
each class.

### <a id="s4.2"></a>4.2 `Ty`

```rust
pub enum Ty { Int, Float, Vec2, Vec3, Vec4, Vec5, String }
```

`Ty` has no boolean. A flag is an `Int` that is 0 or 1 (ADR-0027). `Vec5` is the point of a
`.troybin` curve of a `Vec4` value: a time and 4 components.

### <a id="s4.3"></a>4.3 `Name`

```rust
pub struct Name {
    pub section: String,
    pub name: String,
    pub ty: Option<Ty>,
    pub stale: bool,
}

impl Name {
    pub fn comment(&self) -> Option<&'static str>;
}
```

`name` includes the comment prefix of a commented property. `comment` returns that prefix. `ty` is
`None` for a read without a declared type. `section` and `name` keep the ASCII case of the first
read that produces them ([section 6.3](#s6.3)).

### <a id="s4.4"></a>4.4 `Options`

```rust
pub struct Options {
    pub stale: bool,
    pub commented: bool,
}
```

`Options::default()` sets both fields: every claim is a candidate name. `Options::client()` clears
both: only client reads without a comment prefix name a key. [Section 8](#s8) states the filter.

### <a id="s4.5"></a>4.5 `Names` and `History`

```rust
pub type Names = BTreeMap<IniHash, Vec<Name>>;
pub type History = HashMap<IniHash, Vec<String>>;
```

`Names` holds the named keys in ascending key order. A key without a name is absent. A key with
more than one name has names of equal rank ([section 7](#s7)).

## <a id="s5"></a>5. Naming a file

### <a id="s5.1"></a>5.1 `unhash`

```rust
pub fn unhash(inibin: &Inibin, class: Class, options: Options) -> Names;
```

Runs the recipe of `class` over `inibin`, ranks the claims of each key ([section 7](#s7)), and
removes the claims that `options` exclude ([section 8](#s8)).

### <a id="s5.2"></a>5.2 `unhash_with_history`

```rust
pub fn unhash_with_history(
    inibin: &Inibin,
    class: Class,
    options: Options,
    history: &History,
) -> Names;
```

Names `inibin` as `unhash` does. A recipe that reads a history ([section 6.7](#s6.7)) reads
`history`. `unhash` reads an empty history.

### <a id="s5.3"></a>5.3 `unhash_any`

```rust
pub fn unhash_any(inibin: &Inibin, options: Options) -> (Class, Names);
```

Runs `unhash` with every class of `Class::ALL`. Returns the class that names the most keys, and
its names. On a tie, returns the first such class of `Class::ALL`. A file whose keys no class
names returns `Class::ALL[0]` and empty names.

### <a id="s5.4"></a>5.4 `to_ini`

```rust
pub const UNKNOWN_SECTION: &str = "unknown";

pub fn to_ini(inibin: &Inibin, names: &Names) -> String;
```

Writes `inibin` as ini text. A key with a name is a line `name=value` in the section of its first
name. A value is the text of its `Display` (`inibin.md` [section 6.3](inibin.md#s6.3)). Sections
and the properties of a section are in ascending order of name, ignoring ASCII case, with a run
of digits ordered by its number: `Particle2` before `Particle10`. A blank line separates two
sections.

A key without a name is a line in the section `[unknown]`, after every other section. Its
property name is the **unknown name** of the key: `0x`, the key in 8 hex digits, `_`, and a suffix
of lowercase letters and digits. The key of `unknown*` and the unknown name is the key. The suffix
is the first in order of length, then of the alphabet `a` to `z`, `0` to `9`.

The key of each line of the text is a key of the file, and each key of the file has one line. The
text holds no declared type, no stale flag and no second name of a key.

The search for a suffix splits it into a head and a tail of 3 characters. A table holds the hash
of each of the 46,656 tails from the state 0. SDBM is linear in its state: for each head, the tail
needs the hash `key - state * MULTIPLIER^3`, where `state` is the hash of `unknown*`, the prefix
and the head. A suffix has at most 7 characters for 20,000 random keys
([appendix A](#appendix-a)).

## <a id="s6"></a>6. Reading

### <a id="s6.1"></a>6.1 Keys

A section has a state, the SDBM hash of `section*`. A read hashes the property name from the
state 0 and keeps the multiplier to the power of the length of the name. The key of the name in
the section with the state `s` is `s * power + hash`. SDBM is linear in its state
(`ltk_hash::sdbm::MULTIPLIER`).

A name is a list of parts: strings, integers in decimal, and other names. A read hashes the parts
in order and builds no string. A read allocates the text of a name only for a new claim.

### <a id="s6.2"></a>6.2 Comment prefixes

A read of `section*name` also looks up `section*<prefix>name` for each comment prefix. A copy with
a prefix that the file has gets a claim with the prefix in its name. The claim is stale
([section 2](#s2)).

### <a id="s6.3"></a>6.3 Sections

Two section names with equal states are one section. The ASCII case of a section name does not
change its state. The section keeps the text of the first read that names it. A section name that
is one string, such as a value of the file, loses the whitespace around it.

### <a id="s6.4"></a>6.4 Values

A read returns the value of the key, or nothing for a key that the file lacks. A string read
returns the string of a `Value::String`, and the text of a value of another storage type. The
ini compiler stores a string of digits as a number: the emitter `123451` is an integer value of
`GroupPart1`. An integer read and a float read convert the value as the client does
(`inibin.md` [section 8](inibin.md#s8)).

### <a id="s6.5"></a>6.5 Lists

A recipe reads a numbered list in one of three ways.

- **Counted.** The client reads a count and then items 1 to the count. A recipe reads items 1 to
  99. An item past the count is a stale read. `IdleParticles` and `DefaultAnimations` of a
  character file are counted: the client logs a missing item and reads on.
- **Up to the first gap.** `seq` reads `Item<start>`, `Item<start + 1>`, and so on, up to the
  first index that has neither the key nor a copy with a comment prefix. An index that has only a
  copy with a comment prefix stops the client. The reads after it are stale.
- **While present.** `for i in a.. while ty name` reads up to the first index without the key,
  and runs its block for each index that has the key. A copy with a comment prefix does not
  extend the list.

### <a id="s6.6"></a>6.6 Stale paths

A `stale` block marks its reads as stale. A `stale unless (condition)` block marks its reads as
stale unless the condition holds. A stale block inside a stale block is stale. A read of a key
that a client read claims elsewhere keeps the claim as a client claim.

`has` tests for a key or a copy of it with a comment prefix. It claims the copies with a comment
prefix, and does not claim the key.

### <a id="s6.7"></a>6.7 Troybin parts

The `.troybin` recipe is hand-written. The client reads `GroupPart1` to `GroupPart8`, and the parts
after them up to the first missing one. `GroupPart<i>Type` selects the Simple or the Complex reader
of the emitter that the part names. The recipe reads the emitter with both readers: the reader
that the type does not select is a stale path. A file also holds parts that the client never reads.
The recipe reads them as stale paths: `GroupPart0`, the parts after a gap, parts with a comment
prefix (`'GroupPart3`), and the emitters that a history lists for `GroupPart<i>` or
`'GroupPart<i>`. An emitter that only a history names is read only if the file has one of the
keys that most emitters have: `e-rate`, `p-life`, `e-life`, `rendermode`, `p-texture`, and so on.

## <a id="s7"></a>7. Ranking

The rank of a claim orders client before stale, and a name without a comment prefix before a name
with one:

| Rank | Claim |
| ---- | ----- |
| 0 | client read, no comment prefix |
| 1 | stale read, no comment prefix |
| 2 | client read, comment prefix |
| 3 | stale read, comment prefix |

The names of a key are its claims of the lowest rank, in the order of the reads that produce them.
Two reads of one name in one section, ignoring ASCII case, are one claim. The claim is a client
claim if either read is, and keeps the declared type of the first read.

## <a id="s8"></a>8. Options

`Options` removes claims before ranking. `commented: false` removes every claim with a comment
prefix. `stale: false` removes every claim that is stale ([section 2](#s2)). A key whose claims are
all removed has no name. The options do not change the reads of a recipe.

## <a id="s9"></a>9. The `recipe!` macro

A recipe is written in `recipe!`, a declarative macro over a `Reader`. A statement reads a list of
names with a reader method (`int "A", ("B" i);`), opens a section (`["Data"] { ... }`), loops
(`for i in 1..5`, `for k in ["A", "B"]`, `for i in 1.. while int ("A" i)`), branches (`if let`,
`if (condition)`), binds a value (`let`), reads a list ([section 6.5](#s6.5)), marks a stale path
([section 6.6](#s6.6)), or calls a fragment (`use f(args);`). The documentation of the `recipes`
module lists every statement.

A name is a string literal, a variable, or a parenthesized list of parts:
`("Mastery" i "Level")`. The `name!` macro takes a variable and a part by reference.

The reads of a recipe follow the order of the client loader. A read with more than one name puts
one name on each line.

## <a id="s10"></a>10. Classes

| Class | Files | Recipe |
| ----- | ----- | ------ |
| `Character` | Unit files, skin files, materials, shop events, recommended items | `recipes/character.rs` |
| `Spell` | Spells, buffs, talents, items, item groups | `recipes/spell.rs` |
| `Globals` | `DATA/Globals` files, bot loadouts, item categories | `recipes/globals.rs` |
| `Ui` | Loading screen, HUD, bars, game bar animations | `recipes/ui.rs` |
| `Particles` | Particle settings, `.prtbin` particles | `recipes/particles.rs` |
| `Troybin` | `.troybin` particle systems | `recipes/troybin.rs` |
| `Level` | Level audio, graphics, terrain, timers, navigation | `recipes/level.rs` |
| `Scene` | The scene configuration and the map objects of a level | `recipes/scene.rs` |

A recipe file runs the modules of its research classes in order. A module carries the path
patterns of its research class as comments. The crate does not classify a file by its path.

A unit file and a skin file have one module: the client reads both with one loader. A skin file
holds a subset of the sections of a unit file, and the MeshSkin keys of a skin.

Shipped files hold keys outside the section that the client reads them from. A recipe reads such a
key as a stale read, with a comment that counts the files of the corpus that hold it.

## <a id="s11"></a>11. Testing

- **Unit tests** check the forward hash against `ltk_hash::sdbm`, the decimal bytes of an integer
  part, and the comparison of a claim name with parts.
- **Shipped files.** `tests/fixtures` holds one shipped file of each class except `Scene`, and a
  skin file. A test asserts that `unhash_any` picks the class of the file and that the keys of the
  lines of `to_ini` are the keys of the file, and snapshots the ini text with `insta`.
- **Unknown names.** A unit test asserts that the unknown names of fixed keys hash to the keys.
- **Built files.** Tests build files key by key for the rules of [section 6](#s6) to
  [section 8](#s8): sections named by values, a numeric emitter name, counted lists, stale and
  commented keys, the options, a history, and a `U1` flag. The scene test builds a map object
  list. The smallest shipped scene file has 4,486 keys.

## <a id="s12"></a>12. Rules

| ID  | Rule | Instead of | Why | Spec |
| --- | ---- | ---------- | --- | ---- |
| U1 | A recipe is Rust code over a reader that names each key it reads. | A table from hash to name. | Many section names are values of the file. A table holds only the names of its corpus. | [section 1](#s1); ADR-0028 |
| U2 | The caller names the class, or `unhash_any` tries every class. | Classifying a file by its path. | Shipped files sit outside the folder of their family. | [section 5](#s5), [section 10](#s10) |
| U3 | `Options::default()` names every key that a recipe reads, stale and commented keys included. | Naming only client reads. | A tool that edits or prints a file needs a name for every key. | [section 4.4](#s4.4), [section 8](#s8) |
| U4 | Options remove claims before ranking and do not change the reads. | Options that skip stale paths. | The reads are one walk of the file for every option. The options select among its claims. | [section 8](#s8) |
| U5 | A client claim ranks before a stale claim, and a claim without a comment prefix before one with a prefix. Equal ranks are all kept. | Picking one name per key. | SDBM collides. The recipe cannot tell two names of equal rank apart. | [section 7](#s7) |
| U6 | A read hashes forwards from the section state and allocates only for a new claim. | Formatting each name and hashing it. | A recipe reads thousands of names per file. Most are absent. | [section 6.1](#s6.1) |
| U7 | Every read also looks up its copies with a comment prefix. A copy with a prefix is stale. | Naming only the key. | The ini compiler of old builds stores commented lines as properties. | [section 6.2](#s6.2) |
| U8 | Section names ignore ASCII case, and a one-string section name loses the whitespace around it. | Exact text. | The hash lowercases. Values carry whitespace. | [section 6.3](#s6.3) |
| U9 | A string read returns the text of a value of any storage type. | Only `Value::String`. | The ini compiler stores a string of digits as a number. | [section 6.4](#s6.4) |
| U10 | A counted list reads items 1 to 99, and items past the count are stale. | Reading up to the first gap. A fixed range of client reads. | The client reads items 1 to the count and logs a missing item. | [section 6.5](#s6.5) |
| U11 | `seq` stops at an index with neither the key nor a commented copy. The reads after a commented-only index are stale. | Stopping at the first missing key. | The client stops at the commented line. A file holds the lines after it. | [section 6.5](#s6.5) |
| U12 | `Ty` has no boolean. A flag is an `Int`. | `Ty::Bool`. | The format has no boolean type. | [section 4.2](#s4.2); ADR-0027 |
| U13 | One module reads unit files and skin files. | A module per folder. | The client reads both with one loader. | [section 10](#s10) |
| U14 | The reads of a recipe follow the order of the client loader, one name per line. | Sorted names. | A recipe is checked against the loader it mirrors. | [section 9](#s9) |
| U15 | `to_ini` writes a key without a name under an unknown name that hashes to the key. | A comment with the key. Leaving the key out. | Ini text with unknown names holds every key of the file. | [section 5.4](#s5.4) |
| U16 | `to_ini` orders sections and properties by name, with digit runs by number. | The order of the ini text. Key order. | The file does not record the order of the ini text. Numbered names read in sequence. | [section 5.4](#s5.4) |
| U17 | `to_ini` writes only names and values. | Comments for the declared type, the stale flag and the other names of a key. | Ini text has no place for them. `Names` holds them. | [section 5.4](#s5.4) |

## <a id="appendix-a"></a>Appendix A. Corpus measurements

Measured on 2026-10-25 against the RADS archive at `X:/Archive/lol/RADS` (`inibins.json` and
`troybins.json` manifests), one file per distinct content and class.

**Agreement with the research tool.** Each file is classified by the path patterns of
`inibin-names` and named by both tools with the same class. Equal sets of names ignoring ASCII case,
or one set inside the other, count as the same. The stale flag is not compared.

| Class | Files | Keys | Named by both | Differ | Only research | Only this crate |
| ----- | ----- | ---- | ------------- | ------ | ------------- | --------------- |
| character | 10,956 | 794,939 | 787,930 | 0 | 0 | 0 |
| globals | 529 | 19,668 | 19,612 | 0 | 0 | 0 |
| level | 644 | 30,372 | 30,258 | 0 | 0 | 0 |
| particles | 24 | 779 | 779 | 0 | 0 | 0 |
| scene | 9 | 35,156 | 35,154 | 0 | 0 | 0 |
| spell | 25,254 | 2,687,179 | 2,677,597 | 0 | 0 | 0 |
| troybin | 63,408 | 11,334,732 | 10,488,579 | 0 | 0 | 0 |
| ui | 312 | 58,598 | 58,099 | 0 | 0 | 0 |

**Character files.** Of 7,317 unit files and 2,640 skin files (`DATA/Characters`):

- One skin file holds the `[Data]` stats of a unit: `DATA/Characters/CorkiBomb/skins/base/Base.inibin`.
- 16 unit files hold `IdleParticles*GameplayCollisionRadius` and 21 hold
  `IdleParticles*SelfIllumination`. 1 unit file holds `MeshSkin*GameplayCollisionRadius`. 2 skin
  files hold `ContextualAction*UsesSkinVO`.
- 114 unit files hold 149 `IdleParticles` items past `NumberOfParticles`. 1 skin file holds 1.
  1 unit file holds 1 `DefaultAnimations` item past `NumberOfAnimations`. No file holds an
  `IdleParticles<n>` section.
- `DATA/Particles/ToxicShotAttack.inibin` is a spell file: every key is a `SpellData` key.

**Unknown names.** `to_ini` of a file of 20,000 random unnamed keys, in a release build: every
unknown name hashes to its key, the longest suffix has 7 characters, and the text takes 2.6
seconds.
