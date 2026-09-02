# ltk_meta

Reading, editing and writing League of Legends **property bins** (`.bin`): the hierarchical, hash-keyed configuration format the game uses for almost all of its data - champions, items, UI, VFX, maps.

Two kinds of file share the `.bin` extension and this crate reads both:

- **`PROP`** - a tree of objects with typed properties (`Bin`).
- **`PTCH`** - an override bin: deletions, extra objects and property patch records applied over a base bin (`BinOverride`).

## Public surface

| Item | What it is |
| --- | --- |
| `Bin`, `BinObject` | The object tree: read (`from_reader`), build (`builder()`), edit, write (`to_writer`) |
| `BinStream`, `BinToc`, `ObjectEntry`, `BatchObjects` | Streaming access: mount a file, sweep object descriptors, random access by path hash, and whole batches of them at once - without parsing values |
| `ObjectView`, `PropertyView`, `ValueView` | Zero-copy views over one buffered object: iterate, look up and descend to any depth without materializing anything |
| `ObjectCache`, `NoCache`, `LruObjectCache` | The opt-in lookup cache `BinStream::cached_object` resolves through |
| `Numbering` | Which property-kind numbering a file is being read under, and whether the legacy latch has flipped |
| `property::values`, `PropertyValueEnum`, `PropertyKind` | One typed value struct per wire kind (`I32`, `String`, `Vector3`, `Container`, `Map`, `Struct`, `Embedded`, `Optional`, …) and the enum over them |
| `ValueSlot` | A mutable handle on one value, carrying the kind its holder pins it to - what `resolve_mut` hands back |
| `concrete` | The value model with the metadata parameter pinned - start here - plus the three streaming names Rust cannot infer without it: `BinStream`, `LruObjectCache`, `NoCache` |
| `path::PropertyPath` | Property addressing (`Position.Anchors.Anchor`, `Elements[3]`, `Lookup{"weapon"}`), with `Bin::resolve`, `resolve_mut` and `Bin::patch` |
| `path::ValueShape` | What a value is - kind, item kind, map key kind, embed class - as the resolver's type rule and the streaming header peek both speak it |
| `BinOverride`, `ApplyReport` | PTCH files: read, build, `check` against / `apply` onto a base `Bin` |
| `BinFile`, `BinKind` | Reading a `.bin` when you don't know which kind it is |
| `traits` | `ReadProperty` / `WriteProperty` / `PropertyExt` (serialized size), for generic code over values |
| `Error` | One `thiserror` enum for the whole crate (`miette` diagnostics included) |

Object names and property names are stored as **FNV-1a hashes of the lowercased string** - compute them with [`ltk_hash`](https://crates.io/crates/ltk_hash)'s `fnv1a::hash_lower`. Integer literals and hashes convert freely where a name is expected.

## Reading a bin

```rust
use std::fs::File;
use ltk_meta::Bin;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::open("data.bin")?;
    let bin = Bin::from_reader(&mut file)?;

    println!("depends on: {:?}", bin.dependencies);
    for (path_hash, object) in &bin.objects {
        println!("{path_hash:08x} ({:08x}): {} properties", object.class_hash, object.properties.len());
    }

    Ok(())
}
```

## Streaming a bin

`Bin::from_reader` parses everything. When you only need the object table - harvesting hashes across thousands of files, or pulling one object's facts out of a big bin - `BinStream` mounts the file, reads just the header, and skips object bodies by their size fields. Hand `mount` the bare `File`; buffering is internal.

```rust
use std::fs::File;
use ltk_meta::concrete::BinStream;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = BinStream::mount(File::open("data.bin")?)?;
    println!("version {}, {} objects", stream.version(), stream.class_hashes().len());

    // Sweep the object table: one descriptor per object, no value parsing.
    for entry in stream.entries() {
        let entry = entry?;
        println!("{:08x}: {:08x}", entry.path_hash, entry.class_hash);
    }

    // Random access by path hash, served from the table of contents the sweep built.
    if let Some(mut object) = stream.object(0x4a47c414u32)? {
        println!("{} properties", object.property_count()?);
    }
    Ok(())
}
```

The `entries()` sweep populates a `BinToc` (`Clone`, serializable with the `serde` feature) as a side effect, so `toc()` and `object(hash)` after a sweep cost no further reads.

`Bin::from_reader` is itself `BinStream::mount` plus `into_bin()`, so the streaming surface and the eager tree are one parser and cannot drift.

### Reading inside an object

Descending buffers the object's declared byte range once and views it in place. Iteration, lookup by name hash and descent into nested values are all slice arithmetic from there; nothing decodes until it is touched and nothing allocates until an *owned* value is asked for.

```rust
use std::fs::File;
use ltk_meta::{concrete::BinStream, ValueView};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = BinStream::mount(File::open("data.bin")?)?;
    let mut objects = stream.objects();

    while let Some(mut object) = objects.next()? {
        let view = object.view()?;

        // Every string in the object, borrowed straight out of the buffer.
        for property in view.properties() {
            if let ValueView::String(text) = property?.value_view()? {
                println!("{text}");
            }
        }

        // `Elements[3].Position`, without materializing a single sibling.
        if let Some(elements) = view.property(0x1234_5678u32)? {
            if let ValueView::Container(list) = elements.value_view()? {
                if let Some(ValueView::Embedded(embed)) = list.get(3)? {
                    println!("{:?}", embed.property(0x9abc_def0u32)?.map(|p| p.kind()));
                }
            }
        }
    }
    Ok(())
}
```

`ObjectStream::read()` gives the owned `BinObject` instead, and `BinStream::cached_object()` resolves through an installed `ObjectCache` (`NoCache` by default, `LruObjectCache` shipped) and hands back an `Arc`. `ObjectStream::byte_range()` gives the object's extent in the file, for copying one out verbatim.

### Asking what a value is without reading it

A complex value declares its shape in the few header bytes ahead of its body, so a `PropertyView` can say what it holds - and how many - at skip cost, without descending. `shape()` returns the same `ValueShape` the resolver's type rule uses, so "is this a `Container[ObjectLink]`?" is one comparison rather than a parse.

```rust
use std::fs::File;
use ltk_meta::{concrete::BinStream, PropertyKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = BinStream::mount(File::open("data.bin")?)?;
    let mut objects = stream.objects();

    while let Some(mut object) = objects.next()? {
        let view = object.view()?;

        for property in view.properties() {
            let property = property?;
            let shape = property.shape()?;

            // Every list of object links, and how long each one is.
            if shape.kind == PropertyKind::Container && shape.item_kind == Some(PropertyKind::ObjectLink) {
                // e.g. `1a2b3c4d: Container[ObjectLink] of Some(12)`
                println!("{:08x}: {shape} of {:?}", property.name_hash(), property.item_count()?);
            }
        }
    }
    Ok(())
}
```

`item_count()` answers for containers and maps and is `None` for everything else, an option included - whether an option holds anything is `OptionalView::is_some`.

### Legacy kind numbering

Bins written before `WadChunkLink` existed number the complex kinds one lower. A handle mounts as `Numbering::Current` and stays there until it meets a kind byte that only decodes as `Numbering::Legacy`: that object is re-walked from the bytes already in memory, and the handle is latched for the rest of its life. Nothing is re-read, and a genuinely desynced file can be reinterpreted this way rather than reported as broken - so `numbering()` is how to tell it happened.

```rust
use std::fs::File;
use ltk_meta::{concrete::BinStream, Numbering};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = BinStream::mount(File::open("data.bin")?)?;
    let mut objects = stream.objects();

    while let Some(mut object) = objects.next()? {
        object.view()?;
    }

    if stream.numbering() == Numbering::Legacy {
        println!("this file was written before WadChunkLink existed");
    }
    Ok(())
}
```

A view carries the numbering it was built under, so one handed out before the flip keeps reading the way it started.

### Opening many objects at once

`object(hash)` answers one question per seek. `objects_batch` takes the whole request up front, so a cold handle resolves it during one forward scan that stops at the last hit, and a warm one visits the rows in offset order. Yield order is file order; `missing()` reports the hashes the file does not hold.

```rust
use std::fs::File;
use ltk_meta::concrete::BinStream;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = BinStream::mount(File::open("data.bin")?)?;
    let mut batch = stream.objects_batch([0x4a47c414u32, 0x1a2b3c4du32]);

    while let Some(mut object) = batch.next()? {
        println!("{:08x}: {} properties", object.path_hash(), object.property_count()?);
    }
    println!("not in this bin: {:?}", batch.missing());
    Ok(())
}
```

## Walking a bin

`ltk_meta::walk` is one traversal for both trees. A `Visitor` is called once per node, in pre-order and file order, and answers a `Visit` that continues, prunes a property, stops or aborts. The walk visits every node of an object once, in pre-order and file order, and asks the visitor before entering each property. The visitor is generic over the tree: the same `Census` runs over an owned `Bin` and over a `BinStream`, where nothing is materialised.

```rust
use ltk_hash::BinHash;
use ltk_meta::{
    walk::{Node, TreeValue, Visit, Visitor},
    Error,
};

/// Counts nodes and records the address of every `Struct` of one class.
#[derive(Default)]
struct Census {
    nodes: usize,
    hits: Vec<(BinHash, String)>,
}

impl<'a, V: TreeValue<'a>> Visitor<'a, V> for Census {
    type Error = Error;

    fn enter_node(&mut self, node: &Node<'_, 'a, V>) -> Result<Visit, Error> {
        self.nodes += 1;
        if *node.class_hash() == 0x1e6b_a0c4 {
            self.hits.push((node.object_hash(), node.trail().to_string()));
        }
        Ok(Visit::Continue)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bin = ltk_meta::Bin::from_reader(&mut std::fs::File::open("data.bin")?)?;
    let mut census = Census::default();
    bin.walk(&mut census)?;
    println!("{} nodes, {} hits", census.nodes, census.hits.len());

    let mut stream = ltk_meta::concrete::BinStream::mount(std::fs::File::open("data.bin")?)?;
    let mut census = Census::default();
    stream.walk(&mut census)?;
    Ok(())
}
```

**In parallel.** The walk over one object is sequential by contract: one visitor, pre-order, `Stop` and `Skip` as ordered decisions. Objects are independent of one another, and every view, node and trail type is `Send`. A sweep parallelises across objects with one visitor instance per worker and a reduce at the end. Nothing in the crate schedules this; the split is the caller's.

```rust
let objects: Vec<_> = bin.objects.values().collect();
let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
let per_worker = objects.len().div_ceil(workers).max(1);

let counted = std::thread::scope(|scope| {
    let workers: Vec<_> = objects
        .chunks(per_worker)
        .map(|chunk| {
            scope.spawn(move || {
                let mut census = Census::default();
                for object in chunk {
                    object.walk(&mut census)?;
                }
                Ok::<_, Error>(census)
            })
        })
        .collect();

    let mut all = Census::default();
    for worker in workers {
        let census = worker.join().expect("a worker panicked")?;
        all.nodes += census.nodes;
        all.hits.extend(census.hits);
    }
    Ok::<_, Error>(all)
})?;

println!("{} nodes, {} hits", counted.nodes, counted.hits.len());
```

Across many files the same shape applies one level up: one task per file, each mounting its own `BinStream` and walking it sequentially. The per-object walk is microseconds; decompression and I/O are where a sweep spends its time.

## Creating one programmatically

The `concrete` module pins the metadata parameter, which is what you want unless you are attaching per-node metadata of your own:

```rust
use ltk_meta::concrete::{values, Bin, BinObject};

fn main() {
    let bin = Bin::builder()
        .dependency("common.bin")
        .object(
            BinObject::builder(0x12345678u32, 0xABCDEF00u32) // (path hash, class hash)
                .property(0x1111, values::I32::new(42))
                .property(0x2222, values::String::from("hello"))
                .build(),
        )
        .build();

    assert_eq!(bin.objects.len(), 1);
}
```

Complex values are validated at construction - container items must share one kind, map keys must be a kind that can key a map - so `Map::new` and friends return `Result`:

```rust
use ltk_meta::{concrete::values, PropertyKind};

fn main() -> Result<(), ltk_meta::Error> {
    let list = values::Container::from(vec![values::I32::new(1), values::I32::new(2)]);
    let map = values::Map::new(
        PropertyKind::U32,
        PropertyKind::String,
        vec![(values::U32::new(7).into(), values::String::from("seven").into())],
    )?;

    assert_eq!(list.len(), 2);
    assert_eq!(map.entries().len(), 1);
    Ok(())
}
```

## Editing and writing back

```rust
use std::fs::File;
use std::io::Cursor;
use ltk_meta::{Bin, BinObject};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut bin = Bin::from_reader(&mut File::open("data.bin")?)?;

    bin.add_object(BinObject::new(0x11112222, 0x33334444));
    bin.remove_object(0x55556666);

    let mut output = Cursor::new(Vec::new());
    bin.to_writer(&mut output)?;
    Ok(())
}
```

Round-tripping is a design goal: reading a shipped bin and writing it back reproduces it byte for byte.

## Addressing a property by path

`PropertyPath` is the language a patch record uses to name what it overrides, and it works on any object: `.` steps into structs and embeds, `[n]` indexes containers, `{key}` looks up map entries.

```rust
use std::fs::File;
use ltk_meta::{path::PropertyPath, property::values, Bin};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut bin = Bin::from_reader(&mut File::open("uibase.bin")?)?;
    let anchor = PropertyPath::new("Position.Anchors.Anchor")?;

    println!("{:?}", bin.resolve(0x4a47c414_u32, &anchor)?);

    // `patch` follows the client's type rule: the shape has to match, or nothing changes.
    bin.patch(
        0x4a47c414_u32,
        &anchor,
        values::Vector2::new(glam::Vec2::new(0.0, 1.0)).into(),
    )?;
    Ok(())
}
```

`resolve_mut` is the unchecked way in: it hands back a `ValueSlot` on whatever is there and applies no type rule, where `patch` reproduces the client's. The slot carries the kind its holder pins the value to - a container declares its item kind once, ahead of the values, so `set` refuses a value of another kind there. Editing in place through `as_mut` needs no check at all, because it cannot change the kind.

```rust
use std::fs::File;
use ltk_meta::{path::PropertyPath, property::ValueMut, Bin};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut bin = Bin::from_reader(&mut File::open("uibase.bin")?)?;
    let mut slot = bin.resolve_mut(0x4a47c414_u32, &PropertyPath::new("Elements[0]")?)?;

    println!("pinned to {:?}", slot.pinned_kind()); // whatever the list declared its items to be

    if let ValueMut::I32(count) = slot.as_mut() {
        count.value += 1;
    }
    Ok(())
}
```

## Override bins (`PTCH`)

A patch only means something over the bin it patches. `apply` lays it over one in the order the client does, and reports what it could not apply instead of failing - which is also what the client does with a stale path. `check` answers the same question without touching anything, which is what to ask after a game update.

```rust
use std::fs::File;
use ltk_meta::{Bin, BinOverride};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut base = Bin::from_reader(&mut File::open("uibase.bin")?)?;
    let patch_bin = BinOverride::from_reader(&mut File::open("uiflipped.bin")?)?;

    for patch in &patch_bin.patches {
        // e.g. `4a47c414 Position.Anchors.Anchor = Vector2`
        println!("{:08x} {} = {:?}", patch.object_hash, patch.path, patch.kind());
    }

    let report = patch_bin.check(&base);
    println!("{report}"); // "109 applied (5 inserted), 0 skipped, ..."
    if report.is_clean() {
        patch_bin.apply(&mut base);
    }
    Ok(())
}
```

## Reading a `.bin` of unknown kind

The extension does not say which kind a file is, so either read it as a `BinFile` and match on what came back, or ask `BinKind::identify_from_reader` and call the reader you want - it leaves the magic in place, so the same reader can be handed straight on.

```rust
use std::fs::File;
use ltk_meta::BinFile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match BinFile::from_reader(&mut File::open("unknown.bin")?)? {
        BinFile::Prop(bin) => println!("{} objects", bin.objects.len()),
        BinFile::Override(patch_bin) => println!("{} patches", patch_bin.patches.len()),
    }
    Ok(())
}
```

## Feature flags

- `serde`: `Serialize`/`Deserialize` on the tree, values, paths and reports (also enables it on the `glam` math types they carry).

## Related crates

- [`ltk_ritobin`](../ltk_ritobin): the human-readable text form of these bins, parsing to and from this crate's types.
- [`ltk_hash`](../ltk_hash): the FNV-1a and xxhash functions the formats key everything by.
- [`ltk_wad`](../ltk_wad): WAD archives, where the game's bins actually live.
- [`league-toolkit`](../../): umbrella crate that re-exports everything behind feature flags.

## License

Licensed under either of MIT or Apache-2.0 at your option.
