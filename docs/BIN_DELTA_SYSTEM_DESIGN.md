# Bin Delta/Patching System Design

## Table of Contents

- [Problem Statement](#problem-statement)
- [Design Overview](#design-overview)
- [Data Model](#data-model)
- [Bin Path Language](#bin-path-language)
  - [Syntax](#syntax)
  - [Examples](#examples)
  - [Data Model](#data-model-1)
  - [API](#api)
  - [Usage in Delta System](#usage-in-delta-system)
  - [Usage in CLI Tooling](#usage-in-cli-tooling)
- [Text Format (`.bin.delta`)](#text-format-bindelta)
  - [Syntax](#syntax-1)
  - [Format Rules](#format-rules)
  - [Hash Resolution](#hash-resolution)
  - [Header](#header)
  - [Dependencies](#dependencies)
- [Resolution Engine](#resolution-engine)
  - [Application Algorithm](#application-algorithm)
  - [Multi-Delta Composition](#multi-delta-composition)
  - [Error Handling Strategy](#error-handling-strategy)
- [Crate Architecture](#crate-architecture)
  - [Integration with `ltk_overlay`](#integration-with-ltk_overlay)
  - [Integration with `ltk_modpkg`](#integration-with-ltk_modpkg)
  - [Coexistence with Full Replacements](#coexistence-with-full-replacements)
- [Diff Generation](#diff-generation)
- [Worked Examples](#worked-examples)
- [Migration Path](#migration-path)
- [Editor Support (LSP Extension)](#editor-support-lsp-extension)
  - [Architecture](#architecture)
  - [TextMate Grammar](#textmate-grammar)
  - [LSP Features](#lsp-features)
  - [Shared Infrastructure with Ritobin LSP](#shared-infrastructure-with-ritobin-lsp)
  - [Monaco Support](#monaco-support)
- [Open Questions / Future Considerations](#open-questions--future-considerations)

---

## Problem Statement

Currently, League of Legends mods ship complete `.bin` files as replacements. When the game patches (which happens frequently), these full-file replacements break even if the mod only changed a handful of properties. A mod that retextures a single VFX material ships the *entire* skin `.bin` file (often >2MB of VFX particle definitions), and when Riot updates anything else in that file, the mod's stale copy overwrites the new data.

**Goal**: Design a delta system that lets mod creators express *only the changes they intend*, so deltas can be cleanly applied on top of any game version as long as the targeted objects/properties still exist.

---

## Design Overview

The system has three main components:

1. **`BinDelta`** - An in-memory representation of a set of changes to apply to a `Bin`
2. **Delta text format** - A human-readable/writable format extending ritobin syntax (`.bin.delta` files)
3. **Delta resolution** - The merge engine that applies deltas to base `Bin` files during overlay building

### Key Principles

- **Structural addressing**: Changes target objects by `path_hash` and properties by `name_hash` - never by position/index
- **Minimal surface**: A delta only describes what changed, not the full file
- **Composable**: Multiple deltas can target the same `.bin` file and are applied in mod priority order
- **Fail-safe**: Missing targets produce warnings, not hard failures (the game may have removed something)
- **Human-authored**: The text format is designed for hand-editing, not just machine generation

---

## Data Model

### Core Types

```rust
/// A complete set of changes to apply to a single Bin file.
///
/// The target file is determined by the delta file's placement in the mod
/// directory structure (mirroring the WAD path), just like full .bin replacements.
pub struct BinDelta {
    /// Object-level operations, keyed by object path_hash
    pub object_ops: IndexMap<u32, ObjectOp>,

    /// Dependencies to add to the target bin
    pub add_dependencies: Vec<String>,

    /// Dependencies to remove from the target bin
    pub remove_dependencies: Vec<String>,
}

/// An operation on a single BinObject.
pub enum ObjectOp {
    /// Add a new object (fails/warns if it already exists)
    Add(BinObject),

    /// Remove an object entirely
    Remove,

    /// Modify properties of an existing object
    Modify(ObjectModify),
}

/// A set of property-level modifications to an existing object.
pub struct ObjectModify {
    /// Property operations, keyed by property name_hash
    pub property_ops: IndexMap<u32, PropertyOp>,
}

/// An operation on a single property.
pub enum PropertyOp {
    /// Set property to a new value (add if missing, replace if exists)
    Set(PropertyValueEnum),

    /// Remove the property
    Remove,

    /// Modify the property value in-place (for complex types only)
    Modify(ValueModify),
}

/// In-place modification of a complex property value.
/// This enables surgical edits to nested structures without replacing the whole value.
pub enum ValueModify {
    /// Modify properties within a Struct or Embedded value
    Struct(StructModify),

    /// Modify a Container (ordered list)
    Container(ContainerModify),

    /// Modify an UnorderedContainer
    UnorderedContainer(UnorderedContainerModify),

    /// Modify a Map
    Map(MapModify),

    /// Modify an Optional
    Optional(OptionalModify),
}

/// Modifications to a Struct/Embedded value's properties.
pub struct StructModify {
    pub property_ops: IndexMap<u32, PropertyOp>,
}

/// Identifies a specific item within a container.
pub enum ItemSelector {
    /// Match by index position (fragile across patches, but simple)
    Index(usize),

    /// Match by a property value inside a struct/embedded item.
    /// e.g. match the VfxEmitterDefinitionData where emitterName == "Sparks1"
    /// Multiple matchers are AND'd together.
    MatchProperty(Vec<(u32, PropertyValueEnum)>),

    /// Match by full value equality (works well for primitives)
    Value(PropertyValueEnum),
}

/// Modifications to a Container (ordered list).
pub struct ContainerModify {
    /// Items to append to the end of the container
    pub append: Vec<PropertyValueEnum>,

    /// Items to remove, identified by selector
    pub remove: Vec<ItemSelector>,

    /// Items to insert at specific indices (applied after removes, before appends)
    pub insert: Vec<(usize, PropertyValueEnum)>,

    /// In-place modifications to existing items, identified by selector.
    /// The ValueModify is applied to the matched item (typically Struct for
    /// containers of pointer/embed types).
    pub modify: Vec<(ItemSelector, ValueModify)>,
}

/// Modifications to an UnorderedContainer (set-like).
pub struct UnorderedContainerModify {
    /// Items to add to the container
    pub add: Vec<PropertyValueEnum>,

    /// Items to remove, identified by selector
    pub remove: Vec<ItemSelector>,

    /// In-place modifications to existing items, identified by selector
    pub modify: Vec<(ItemSelector, ValueModify)>,
}

/// Modifications to a Map.
pub struct MapModify {
    /// Entries to add or replace (key -> value)
    pub set: Vec<(PropertyValueEnum, PropertyValueEnum)>,

    /// Keys to remove
    pub remove: Vec<PropertyValueEnum>,

    /// Values to modify in-place by key (for complex map values)
    pub modify: Vec<(PropertyValueEnum, ValueModify)>,
}

/// Modifications to an Optional value.
pub enum OptionalModify {
    /// Set the optional to Some(value)
    Set(PropertyValueEnum),

    /// Clear the optional to None
    Clear,
}
```

---

## Bin Path Language

A path expression that uniquely addresses any value within a `Bin` tree. Useful for:
- Delta system: compact target addressing and conflict detection
- Tooling: querying/inspecting bin files from CLI
- Error messages: describing exactly where a problem occurred
- Conflict reporting: showing which paths two mods both modify

### Syntax

A bin path is a chain of **segments** separated by `.`. The object is specified externally (by the delta's `~object` directive, or by a CLI argument) — the path addresses properties *within* an object.

```
<property>[.<navigation>]*
```

#### Segments

| Segment | Syntax | Description |
|---------|--------|-------------|
| Property | `samplerValues` | Property by name (resolved to name_hash) |
| Property (hash) | `#0x1234ABCD` | Property by raw name_hash |
| Index | `[0]` | Container/list item by index |
| Match | `[emitterName="Sparks1"]` | Container item by property match |
| Multi-match | `[name="Bloom",type="float"]` | Container item by multiple property matches (AND) |
| Map key | `{"NUM_BLEND_WEIGHTS"}` | Map entry by string key |
| Map key (hash) | `{0xDEADBEEF}` | Map entry by hash key |
| Map key (int) | `{42}` | Map entry by integer key |
| Optional inner | `?` | Unwrap optional value |

#### Grammar

```
bin_path     = property_seg navigation*
property_seg = ident | '#' hex_hash
navigation   = '.' ident           -- struct/embed property access
             | '.' '#' hex_hash    -- struct/embed property by hash
             | '[' index ']'       -- container index
             | '[' matcher ']'     -- container match
             | '{' key '}'         -- map key
             | '.?'                -- optional unwrap
index        = integer
matcher      = match_expr (',' match_expr)*
match_expr   = ident '=' literal
key          = string_literal | hex_hash | integer
```

### Examples

Paths within objects from a champion skin `.bin`:

```python
# The skin's champion name property
championSkinName

# The diffuse texture path in the VFX base material
samplerValues[TextureName="Diffuse_Texture"].texturePath

# Bloom intensity shader parameter value
paramValues[name="Bloom_Intensity"].value

# A shader macro in the material's map
shaderMacros{"NUM_BLEND_WEIGHTS"}

# The blend mode of a specific VFX emitter (match by name)
complexEmitterDefinitionData[emitterName="Sparks1"].blendMode

# The constant value of the rate in the first emitter (index-based)
complexEmitterDefinitionData[0].rate.constantValue

# Deep nesting: noise field radius inside a specific emitter
complexEmitterDefinitionData[emitterName="Sparks1"].fieldCollectionDefinition.fieldNoiseDefinitions[0].radius.constantValue

# Material technique pass shader link
techniques[0].passes[0].shader

# Optional value unwrap
iconCircle.?

# Raw hash addressing (when names are unknown)
#0x1234ABCD[0].#0x5678EF00
```

### Data Model

```rust
/// A parsed path expression addressing a value within a BinObject.
///
/// The object itself is identified externally (e.g. by the delta's ~object
/// directive or a CLI argument). BinPath addresses properties within it.
pub struct BinPath {
    /// Chain of navigation segments from the object root to the target value
    pub segments: Vec<PathSegment>,
}

/// A single navigation step within the bin tree.
pub enum PathSegment {
    /// Access a property by name or hash
    Property(PropertySelector),

    /// Access a container item by index
    Index(usize),

    /// Access a container item by property match
    Match(Vec<(u32, PropertyValueEnum)>),

    /// Access a map entry by key
    MapKey(PropertyValueEnum),

    /// Unwrap an optional value
    OptionalUnwrap,
}

/// Selects a property by name or hash.
pub enum PropertySelector {
    Name(String),
    Hash(u32),
}
```

### API

```rust
/// Parse a path expression string.
pub fn parse(input: &str) -> Result<BinPath, PathParseError>;

/// Format a path back to string (requires hash provider for readable output).
pub fn display(path: &BinPath, hashes: &impl HashProvider) -> String;

/// Resolve a path against a BinObject, returning a reference to the value.
pub fn resolve<'a>(object: &'a BinObject, path: &BinPath) -> Result<&'a PropertyValueEnum, ResolveError>;

/// Resolve a path mutably.
pub fn resolve_mut<'a>(object: &'a mut BinObject, path: &BinPath) -> Result<&'a mut PropertyValueEnum, ResolveError>;

/// Check if two paths potentially conflict (address the same or overlapping values).
pub fn conflicts(a: &BinPath, b: &BinPath) -> bool;
```

### Usage in Delta System

Bin paths give us a compact way to describe delta targets and detect conflicts:

```rust
// Conflict detection during multi-mod composition
let mod_a_touches: Vec<BinPath> = delta_a.touched_paths();
let mod_b_touches: Vec<BinPath> = delta_b.touched_paths();

for a in &mod_a_touches {
    for b in &mod_b_touches {
        if conflicts(a, b) {
            warn!("Mods conflict at: {}", display(a, &hashes));
        }
    }
}
```

### Usage in CLI Tooling

```bash
# Query a value from a bin file (object specified separately)
$ ltk bin get skin0.bin --object "Characters/Zaahen/Skins/Skin0" "championSkinName"
"ZaahenBase"

# Query a nested VFX property
$ ltk bin get skin0.bin --object "Characters/Zaahen/Skins/Skin0/Particles/Zaahen_Base_Q_1_Buff" \
    'complexEmitterDefinitionData[emitterName="Sparks1"].blendMode'
1

# List all objects in a bin file
$ ltk bin objects skin0.bin --filter "*/Materials/*"
Characters/Zaahen/Skins/Skin0/Materials/Zaahen_VFXBase_inst
Characters/Zaahen/Skins/Skin0/Materials/Zaahen_VFXBase_Wings_inst
```

---

## Text Format (`.bin.delta`)

The delta text format extends ritobin syntax with delta-specific directives. Files use the `.bin.delta` extension (or `.ritobin.delta` for the text form).

### Syntax

```
#DELTA

# ─── Object-level operations ───

# Add a new object
+object 0xAABBCCDD : SomeClassName {
    someProperty: f32 = 1.5
    anotherProp: string = "hello"
}

# Remove an object
-object 0x11223344

# Modify an existing object's properties
~object 0x55667788 {
    # Set a property (creates if missing, overwrites if present)
    =mSpellDamage: f32 = 80.0

    # Remove a property
    -mOldProperty

    # Modify a nested struct in-place
    ~mSpellData {
        =mCooldown: f32 = 8.0
        =mRange: f32 = 600.0
    }

    # Modify a container (ordered list)
    ~mEffects: list[embed] {
        # Append items to end
        +append SpellEffect {
            mEffectName: string = "NewEffect"
            mDuration: f32 = 2.0
        }

        # Insert at a specific index
        +insert[0] SpellEffect {
            mEffectName: string = "FirstEffect"
            mDuration: f32 = 1.0
        }

        # Remove by value match (for primitives)
        -remove "OldEffectValue"

        # Remove by index
        -remove[3]

        # Remove by property match (for struct/embed/pointer items)
        -match(mEffectName = "OldEffect")

        # Modify an item in-place by property match (preferred - patch resilient)
        ~match(emitterName = "Sparks1") {
            =pass: i16 = 100
            =blendMode: u8 = 2
        }

        # Modify an item in-place by index (fragile but simple)
        ~[0] {
            =rate: embed = ValueFloat {
                constantValue: f32 = 20
            }
        }

        # Multiple match criteria (AND'd together)
        ~match(emitterName = "Trail", pass = 80) {
            =pass: i16 = 90
        }
    }

    # Modify an unordered container (set)
    ~mTags: list2[hash] {
        +add 0xDEADBEEF
        -remove 0xCAFEBABE
    }

    # Modify a map
    ~mStatScaling: map[hash, f32] {
        # Set/replace entries
        =0xAAAA: 1.5
        =0xBBBB: 2.0

        # Remove entries by key
        -0xCCCC

        # Modify a complex map value in-place
        ~0xDDDD {
            =mInnerProp: i32 = 42
        }
    }

    # Modify an optional
    ~mOptionalStruct: option[embed] {
        =set SomeClass {
            mValue: i32 = 10
        }
    }

    # Clear an optional to None
    ~mOtherOptional: option[i32] {
        =clear
    }
}
```

### Format Rules

| Prefix | Meaning | Context |
|--------|---------|---------|
| `+object` | Add new object | Top-level |
| `-object` | Remove object | Top-level |
| `~object` | Modify object | Top-level |
| `=property` | Set property value | Inside `~object` or `~struct` |
| `-property` | Remove property | Inside `~object` or `~struct` |
| `~property` | Modify property in-place | Inside `~object` or `~struct` |
| `+append` | Append to container | Inside `~list` |
| `+insert[N]` | Insert at index N | Inside `~list` |
| `-remove` | Remove by value equality | Inside `~list` or `~list2` |
| `-remove[N]` | Remove by index | Inside `~list` |
| `-match(k=v)` | Remove by property match | Inside `~list` or `~list2` |
| `~match(k=v)` | Modify item by property match | Inside `~list` or `~list2` |
| `~[N]` | Modify item by index | Inside `~list` or `~list2` |
| `+add` | Add to unordered container | Inside `~list2` |
| `=key: value` | Set map entry | Inside `~map` |
| `-key` | Remove map entry | Inside `~map` |
| `~key` | Modify map value in-place | Inside `~map` |
| `=set` | Set optional value | Inside `~option` |
| `=clear` | Clear optional | Inside `~option` |

### Hash Resolution

Like ritobin, the format supports both raw hex hashes and named identifiers:
- `0xAABBCCDD` - raw hash literal
- `SomeClassName` - resolved via hash tables (FNV-1a of lowercase)
- Mixing is allowed: `~object 0xAABB { =mPropertyName: f32 = 1.0 }`

### Header

Every delta file starts with `#DELTA` (analogous to ritobin's `#PROP_text`). The target `.bin` file is determined by the delta file's placement in the mod directory structure, mirroring the WAD path - the same convention used for full `.bin` replacements.

### Dependencies

```
#DELTA

+dependency "some/new/dependency.bin"
-dependency "some/old/dependency.bin"
```

---

## Resolution Engine

### Application Algorithm

```
fn apply_delta(base: &mut Bin, delta: &BinDelta) -> DeltaResult {
    let mut warnings = Vec::new();

    // 1. Apply dependency changes
    for dep in &delta.add_dependencies {
        if !base.dependencies.contains(dep) {
            base.add_dependency(dep);
        }
    }
    for dep in &delta.remove_dependencies {
        base.dependencies.retain(|d| d != dep);
    }

    // 2. Apply object operations in order
    for (path_hash, op) in &delta.object_ops {
        match op {
            ObjectOp::Add(obj) => {
                if base.contains_object(*path_hash) {
                    warnings.push(Warning::ObjectAlreadyExists(*path_hash));
                }
                base.add_object(obj.clone());
            }
            ObjectOp::Remove => {
                if base.remove_object(*path_hash).is_none() {
                    warnings.push(Warning::ObjectNotFound(*path_hash));
                }
            }
            ObjectOp::Modify(modify) => {
                match base.get_object_mut(*path_hash) {
                    Some(obj) => apply_object_modify(obj, modify, &mut warnings),
                    None => warnings.push(Warning::ObjectNotFound(*path_hash)),
                }
            }
        }
    }

    DeltaResult { warnings }
}
```

### Multi-Delta Composition

When multiple mods target the same `.bin` file, their deltas are applied sequentially in **mod priority order** (lowest priority first). This means:

1. Base game `.bin` is loaded from WAD
2. Mod A's delta (priority 0) is applied
3. Mod B's delta (priority 10) is applied on top
4. Result is written to overlay WAD

**Conflict detection**: The overlay builder tracks which `(object, property)` pairs each mod touches. If two mods modify the same property, a conflict is reported to the user. The higher-priority mod wins.

### Error Handling Strategy

| Condition | Behavior |
|-----------|----------|
| Target object not found for `Modify` | Warning, skip operation |
| Target object not found for `Remove` | Warning, skip |
| Target object exists for `Add` | Warning, overwrite |
| Target property not found for `Modify` | Warning, skip |
| Target property not found for `Remove` | Warning, skip |
| Type mismatch (e.g., modify struct on f32) | Error, abort delta |
| Container item not found for `-remove` | Warning, skip that item |
| Container `~match()` finds no match | Warning, skip |
| Container `~match()` finds multiple matches | Warning, apply to first |
| Container `~[N]` index out of bounds | Warning, skip |
| Map key not found for `Modify` | Warning, skip |

Warnings are collected and surfaced to the mod creator/user. The overlay builder decides whether to proceed or abort based on severity.

---

## Crate Architecture

### New crate: `ltk_bin_delta`

```
crates/ltk_bin_delta/
├── src/
│   ├── lib.rs          # Public API: apply(), parse(), write()
│   ├── types.rs        # BinDelta, ObjectOp, PropertyOp, ValueModify, etc.
│   ├── apply.rs        # Resolution engine (apply delta to Bin)
│   ├── diff.rs         # Diff engine (compute delta between two Bins)
│   ├── parse.rs        # Text format parser (.bin.delta -> BinDelta)
│   ├── write.rs        # Text format writer (BinDelta -> .bin.delta)
│   ├── error.rs        # Error and warning types
│   └── result.rs       # DeltaResult with warnings
├── tests/
│   ├── apply.rs        # Round-trip and edge case tests
│   ├── parse.rs        # Parser tests
│   └── diff.rs         # Diff algorithm tests
└── Cargo.toml
```

**Dependencies**: `ltk_meta`, `ltk_hash`, `ltk_ritobin` (for hash resolution), `indexmap`, `thiserror`, `miette` (for diagnostic errors with spans)

### Public API

```rust
// Core operations
pub fn apply(base: &mut Bin, delta: &BinDelta) -> DeltaResult;
pub fn diff(old: &Bin, new: &Bin) -> BinDelta;

// Text format
pub fn parse(input: &str) -> Result<BinDelta, ParseError>;
pub fn write(delta: &BinDelta) -> Result<String, WriteError>;
pub fn write_with_hashes(delta: &BinDelta, hashes: &impl HashProvider) -> Result<String, WriteError>;
```

### Integration with `ltk_overlay`

The overlay builder gains a new processing step between collection and WAD building:

```
Current pipeline:
  Index -> Collect overrides -> Distribute -> Build WADs

New pipeline:
  Index -> Collect overrides -> Collect deltas -> Distribute -> Resolve deltas -> Build WADs
                                     ^                              ^
                                     |                              |
                              .bin.delta files              apply() on base bins
```

When the overlay builder encounters a `.bin.delta` file in a mod:
1. Hash the target path to find which WAD(s) contain the target `.bin`
2. Load the base `.bin` from the game WAD
3. Apply all deltas targeting that file (in priority order)
4. Write the merged result as a full `.bin` into the overlay WAD

### Integration with `ltk_modpkg`

The mod package format recognizes `.bin.delta` files as a distinct content type. During packing:
- `.bin.delta` text files are parsed and validated
- They can optionally be compiled to a compact binary delta representation
- During unpacking/overlay building, they are distinguished from full `.bin` replacements

### Coexistence with Full Replacements

A mod can ship both full `.bin` replacements and `.bin.delta` files. The overlay builder applies them as:
1. If a mod ships a full `.bin` for a path, that replaces the base entirely
2. If a mod ships a `.bin.delta` for a path, it's applied on top of whatever the current state is (base or previously replaced)
3. A mod should not ship both a full `.bin` and a `.bin.delta` for the same target path (warning)

---

## Diff Generation

The `diff()` function enables tooling to auto-generate deltas:

```rust
pub fn diff(old: &Bin, new: &Bin) -> BinDelta {
    // 1. Objects in `new` but not in `old` -> ObjectOp::Add
    // 2. Objects in `old` but not in `new` -> ObjectOp::Remove
    // 3. Objects in both -> compare properties:
    //    a. Properties in `new` but not `old` -> PropertyOp::Set
    //    b. Properties in `old` but not `new` -> PropertyOp::Remove
    //    c. Properties in both with different values -> PropertyOp::Set
    //       (or PropertyOp::Modify for complex types if beneficial)
}
```

This allows a workflow where a modder:
1. Extracts the original `.bin` from the game
2. Edits it with existing tools (or converts to ritobin, edits, converts back)
3. Runs `diff(original, modified)` to produce a minimal delta
4. Ships the delta instead of the full file

The diff engine uses `PropertyOp::Set` for primitive changes and generates `ValueModify` operations for complex types where it would significantly reduce delta size (e.g., modifying one property in a struct with 20 properties).

---

## Worked Examples

### Example 1: Retexturing a VFX material

A mod that swaps the diffuse texture on Zaahen's VFX base material:

```
#DELTA

~object Characters/Zaahen/Skins/Skin0/Materials/Zaahen_VFXBase_inst {
    ~samplerValues: list2[embed] {
        ~match(TextureName = "Diffuse_Texture") {
            =texturePath: string = "ASSETS/Characters/Zaahen/Skins/Base/Zaahen_Custom_TX_CM.Zaahen.tex"
        }
    }
}
```

This survives any game patch that doesn't restructure the material definition itself - even if Riot adds new shader parameters or changes other samplers.

### Example 2: Modifying a VFX emitter

A mod that changes the color and blend mode of a specific particle emitter in Zaahen's Q buff effect:

```
#DELTA

~object Characters/Zaahen/Skins/Skin0/Particles/Zaahen_Base_Q_1_Buff {
    ~complexEmitterDefinitionData: list[pointer] {
        ~match(emitterName = "Sparks1") {
            =blendMode: u8 = 2
            =Color: embed = ValueColor {
                dynamics: pointer = VfxAnimatedColorVariableData {
                    times: list[f32] = {
                        0
                        0.5
                    }
                    values: list[vec4] = {
                        { 0.2, 0.6, 1.0, 0 }
                        { 0.1, 0.3, 0.8, 1 }
                    }
                }
            }
        }
    }
}
```

### Example 3: Adding a new particle object

```
#DELTA

+object Characters/Zaahen/Skins/Skin0/Particles/Zaahen_Custom_Idle_Glow : VfxSystemDefinitionData {
    complexEmitterDefinitionData: list[pointer] = {
        VfxEmitterDefinitionData {
            rate: embed = ValueFloat {
                constantValue: f32 = 5
            }
            particleLifetime: embed = ValueFloat {
                constantValue: f32 = 2.0
            }
            emitterName: string = "IdleGlow"
            blendMode: u8 = 1
        }
    }
    particleName: string = "Zaahen_Custom_Idle_Glow"
    particlePath: string = "Characters/Zaahen/Skins/Skin0/Particles/Zaahen_Custom_Idle_Glow"
}
```

### Example 4: Multi-mod composition

Mod A (priority 0) - Custom material shader params:
```
#DELTA

~object Characters/Zaahen/Skins/Skin0/Materials/Zaahen_VFXBase_inst {
    ~paramValues: list2[embed] {
        ~match(name = "Bloom_Intensity") {
            =value: vec4 = { 2.5, 0, 0, 0 }
        }
    }
}
```

Mod B (priority 10) - Custom textures (same file, different properties):
```
#DELTA

~object Characters/Zaahen/Skins/Skin0/Materials/Zaahen_VFXBase_inst {
    ~samplerValues: list2[embed] {
        ~match(TextureName = "Gradient_Texture") {
            =texturePath: string = "ASSETS/Characters/Zaahen/Skins/Base/Particles/Zaahen_Custom_Gradient.Zaahen.tex"
        }
    }
}
```

Both apply cleanly because they touch different properties on the same object. The overlay builder applies A first, then B, producing a merged bin with both changes.

---

## Migration Path

1. **Phase 1**: Implement `ltk_bin_delta` with `apply()`, `parse()`, `write()`, and `diff()`
2. **Phase 2**: Add `.bin.delta` support to `ltk_modpkg` packing/unpacking
3. **Phase 3**: Integrate delta resolution into `ltk_overlay` builder pipeline
4. **Phase 4**: Add delta generation tooling to `league-mod` CLI (`league-mod diff old.bin new.bin`)
5. **Phase 5**: Add conflict detection and reporting in `ltk-manager` UI

### Backwards Compatibility

- Mods using full `.bin` replacements continue to work unchanged
- The `.bin.delta` format is purely additive - no breaking changes to existing formats
- Old mod managers that don't understand `.bin.delta` files will simply ignore them (and should warn)

---

## Editor Support (LSP Extension)

The `.bin.delta` format is designed to be hand-authored, so editor support is essential. The existing ritobin LSP prototype (for `.py`/`.ritobin` files) can be extended to support the delta format with syntax highlighting, diagnostics, completions, and hover info.

### Architecture

The LSP extension is split into two parts:

1. **Language Server** (Rust) — handles parsing, validation, and semantic analysis
2. **VS Code Extension** (TypeScript) — client that communicates with the server + TextMate grammar for syntax highlighting

```
┌─────────────────────┐      LSP/JSON-RPC       ┌──────────────────────┐
│   VS Code Client    │◄────────────────────────►│   Language Server    │
│                     │                          │                      │
│  - TextMate grammar │                          │  - Delta parser      │
│  - Extension config │                          │  - Ritobin parser    │
│  - Commands         │                          │  - Hash tables       │
│                     │                          │  - Bin path resolver  │
└─────────────────────┘                          └──────────────────────┘
```

The server handles both `.py`/`.ritobin` (existing) and `.bin.delta` (new) file types. Sharing the server means the ritobin value parser, hash tables, and type knowledge are reused directly.

### TextMate Grammar

A TextMate grammar (`bindelta.tmLanguage.json`) provides syntax highlighting for the delta format. The key token scopes:

```json
{
  "scopeName": "source.bindelta",
  "fileTypes": ["bin.delta"],
  "patterns": [
    { "match": "^#DELTA", "name": "keyword.control.header.bindelta" },
    { "match": "^#.*$", "name": "comment.line.bindelta" },

    { "match": "\\+object", "name": "keyword.operator.add.bindelta" },
    { "match": "-object", "name": "keyword.operator.remove.bindelta" },
    { "match": "~object", "name": "keyword.operator.modify.bindelta" },

    { "match": "\\+append", "name": "keyword.operator.add.bindelta" },
    { "match": "\\+insert\\[\\d+\\]", "name": "keyword.operator.add.bindelta" },
    { "match": "\\+add", "name": "keyword.operator.add.bindelta" },
    { "match": "\\+dependency", "name": "keyword.operator.add.bindelta" },

    { "match": "-remove", "name": "keyword.operator.remove.bindelta" },
    { "match": "-remove\\[\\d+\\]", "name": "keyword.operator.remove.bindelta" },
    { "match": "-match\\([^)]+\\)", "name": "keyword.operator.remove.bindelta" },
    { "match": "-dependency", "name": "keyword.operator.remove.bindelta" },

    { "match": "~match\\([^)]+\\)", "name": "keyword.operator.modify.bindelta" },
    { "match": "~\\[\\d+\\]", "name": "keyword.operator.modify.bindelta" },

    { "match": "=set|=clear", "name": "keyword.operator.set.bindelta" },
    { "match": "^\\s*=\\w+", "name": "keyword.operator.set.bindelta" },
    { "match": "^\\s*-\\w+", "name": "keyword.operator.remove.bindelta" },
    { "match": "^\\s*~\\w+", "name": "keyword.operator.modify.bindelta" }
  ]
}
```

**Color semantics** (mapping to typical VS Code themes):
- `+` operations (add/append/insert) → **green** (keyword.operator.add)
- `-` operations (remove/delete) → **red** (keyword.operator.remove)
- `~` operations (modify) → **yellow/orange** (keyword.operator.modify)
- `=` operations (set) → **blue** (keyword.operator.set)
- Object/class names → **type color** (entity.name.type)
- Property names → **variable color** (variable.other.property)
- Hash literals (`0xAABB`) → **constant color** (constant.numeric.hex)
- String values → **string color** (string.quoted)
- Type annotations (`f32`, `list[embed]`) → **type color** (support.type)

This gives modders immediate visual feedback on what each line does — adds are green, removes are red, modifications are yellow.

### LSP Features

#### Diagnostics (Errors & Warnings)

Real-time validation as the user types:

| Diagnostic | Severity | Example |
|-----------|----------|---------|
| Invalid header (not `#DELTA`) | Error | `#PROP_text` in a `.bin.delta` file |
| Unknown type name | Error | `=prop: float32 = 1.0` (should be `f32`) |
| Invalid hash literal | Error | `0xZZZZ` |
| Unclosed block | Error | Missing `}` |
| Unknown operation prefix | Error | `*object` (not `+`, `-`, or `~`) |
| Type mismatch in match | Warning | `~match(emitterName = 42)` when emitterName is string |
| Duplicate operation on same target | Warning | Two `=mProp` lines in same object |
| Unknown property name (with hash tables) | Info | `=mNonExistentProp` not found in known hashes |
| Unknown class name (with hash tables) | Info | `+object X : UnknownClass` |

The parser already tracks spans via `nom_locate`, so error locations map directly to LSP `Diagnostic` ranges.

#### Completion

Context-aware suggestions:

- **After `+object ... : `** → known class names from hash tables (e.g. `VfxSystemDefinitionData`, `StaticMaterialDef`)
- **After `~object `** → object paths from the target `.bin` file (if available in workspace)
- **After `=` inside an object** → property names known for that object's class
- **After `~match(`** → property names valid for the container's item type
- **Type positions** → all valid type names (`f32`, `i32`, `string`, `list`, `embed`, etc.)
- **After `list[`/`option[`** → valid inner types
- **After `map[`** → valid key types, then valid value types

This requires hash table files to be loaded. The server looks for `hashes.bin*.txt` files in the workspace or a configured path.

#### Hover

Show information when hovering over identifiers:

- **Object paths** → show resolved path_hash, class name if known
- **Property names** → show resolved name_hash, expected type if known from class schema
- **Class names** → show resolved class_hash, known properties
- **Hash literals** → show resolved name if found in hash tables
- **Type names** → show description and valid value format
- **`~match()` expressions** → show the bin path being addressed

#### Go to Definition / Peek

- **Object links** (`link = "Characters/Zaahen/..."`) → jump to that object's definition in the same or linked `.bin` file
- **`~object` paths** → jump to the corresponding object in the base `.bin`/`.py` file if present in workspace

#### Code Actions

- **"Convert to delta"** — on a `.py`/`.ritobin` file, offer to generate a `.bin.delta` by diffing against the base game file
- **"Expand match to index"** — convert a `~match()` to `~[N]` by resolving against the base file
- **"Add missing properties"** — when a `+object` is missing required properties for its class, offer to scaffold them

### Shared Infrastructure with Ritobin LSP

The delta LSP reuses most of the ritobin LSP's infrastructure:

| Component | Reused from ritobin LSP | Delta-specific |
|-----------|------------------------|----------------|
| Value parser (strings, numbers, vectors, etc.) | Yes | — |
| Type name resolution | Yes | — |
| Hash table loading & lookup | Yes | — |
| Span tracking & error reporting | Yes | — |
| TextMate grammar for values | Embedded | Delta operation prefixes |
| Delta operation parser | — | New |
| Bin path parser | — | New |
| Match expression parser | — | New |
| Delta-specific diagnostics | — | New |
| Delta-specific completions | — | New |

### File Associations

```json
{
  "languages": [
    {
      "id": "ritobin",
      "aliases": ["Ritobin"],
      "extensions": [".py"],
      "configuration": "./language-configuration.json"
    },
    {
      "id": "bindelta",
      "aliases": ["Bin Delta"],
      "extensions": [".bin.delta"],
      "configuration": "./delta-language-configuration.json"
    }
  ]
}
```

The `.py` extension conflict with Python is handled by file content detection (checking for `#PROP_text` header) or by workspace-level association settings.

### Monaco Support

For web-based editors (e.g. a future LTK Manager built-in editor), the same grammar and language logic can target Monaco:

- TextMate grammars work in Monaco via `monaco-textmate` + `vscode-oniguruma`
- The LSP server can run as a WebSocket service or be compiled to WASM
- Alternatively, a lightweight Monaco `IMonarchLanguage` tokenizer can be written for syntax highlighting only, with the full LSP for richer features

---

## Open Questions / Future Considerations

1. **Binary delta format**: Should there be a compact binary encoding of `BinDelta` for distribution, or is text-only sufficient? Text is human-readable and diffs well in VCS, but adds parsing overhead. Given `.bin` files are typically small (<100KB), text is likely fine.

2. **Conditional operations**: Should deltas support conditions? E.g., "only apply this change if property X has value Y". This adds complexity but could improve cross-version compatibility. **Recommendation**: defer to a future version.

3. **Match-by-property ambiguity**: When `~match(emitterName = "Sparks1")` matches multiple items, the engine should apply to the first match and warn. Alternatively, it could apply to all matches. **Recommendation**: apply to first match only, require users to add more match criteria to disambiguate. Emit a warning when multiple matches exist.

4. **Relation to Riot's PTCH format**: `ltk_meta` already reads `PTCH`-prefixed bin files (`Bin::PTCH`), which appear to be Riot's own override mechanism. The `data_overrides` field is currently unimplemented. Our delta system is independent and operates at a higher level (tooling layer, not file format layer), but understanding PTCH better could inform future optimizations.

5. **Delta validation**: Should the parser validate that referenced types exist in known hash tables? E.g., warn if an object references class hash `0xDEAD` which doesn't correspond to any known class. **Recommendation**: optional validation pass, not required for parsing.
