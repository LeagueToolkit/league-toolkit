/*!
Types for reading and writing League of Legends
property bin files (`.bin`).

Property bins are hierarchical data structures used throughout League's
game data. They contain objects with typed properties that can reference
other objects and external files.

## Quick Start

### Reading a bin file

```no_run
use std::fs::File;
use ltk_meta::Bin;

let mut file = File::open("data.bin")?;
let tree = Bin::from_reader(&mut file)?;

for (path_hash, object) in &tree.objects {
    println!("Object {:08x} has {} properties", path_hash, object.properties.len());
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Creating a bin file programmatically

The [`concrete`] module pins the metadata parameter to [`NoMeta`], so nothing
generic needs spelling:

```
use ltk_meta::concrete::{values, Bin, BinObject};

// Using the builder pattern
let tree = Bin::builder()
    .dependency("common.bin")
    .object(
        BinObject::builder(0x12345678u32, 0xABCDEF00u32)
            .property(0x1111, values::I32::new(42))
            .property(0x2222, values::String::from("hello"))
            .build()
    )
    .build();

// Or using the simple constructor
let tree = Bin::new(
    [BinObject::new(0x1234u32, 0x5678u32)],
    ["dependency.bin"],
);
```

[`NoMeta`]: crate::property::NoMeta

### Streaming a bin file

Reading everything is the wrong shape for harvesting hashes across thousands of files or
peeking at one header. The [`stream`] module mounts a `PROP` file the way `ltk_wad::Wad`
mounts an archive and reads only what is asked for:

```no_run
use std::fs::File;
use ltk_meta::concrete::BinStream;

let mut stream = BinStream::mount(File::open("data.bin")?)?;
println!("version {}, {} objects", stream.version(), stream.class_hashes().len());

for entry in stream.entries() {
    let entry = entry?;
    println!("{:08x}: {:08x}", entry.path_hash, entry.class_hash);
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

Descending into an object buffers its declared byte range once and views it in place, so
iteration and descent to any depth cost no I/O and materialize nothing:

```no_run
use std::fs::File;
use ltk_meta::{concrete::BinStream, stream::ValueView};

let mut stream = BinStream::mount(File::open("data.bin")?)?;
let mut objects = stream.objects();

while let Some(mut object) = objects.next()? {
    for property in object.view()?.properties() {
        if let ValueView::String(text) = property?.value_view()? {
            println!("{text}");
        }
    }
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

[`Bin::from_reader`] is itself `BinStream::mount` plus [`BinStream::into_bin`], so the eager
tree and the streaming surface are one parser and cannot drift.

### Modifying a bin file

```no_run
use std::fs::File;
use std::io::Cursor;
use ltk_meta::{Bin, BinObject};

let mut file = File::open("data.bin")?;
let mut tree = Bin::from_reader(&mut file)?;

// Add a new object
tree.add_object(BinObject::new(0x11112222, 0x33334444));

// Remove an object
tree.remove_object(0x55556666);

// Write back
let mut output = Cursor::new(Vec::new());
tree.to_writer(&mut output)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Reading an override bin

Some bins are `PTCH` files: a patch of deletions, extra objects and property patch
records applied over one base bin. They are [`BinOverride`], not [`Bin`].

```no_run
use std::fs::File;
use ltk_meta::BinOverride;

let mut file = File::open("uiflipped.bin")?;
let patch_bin = BinOverride::from_reader(&mut file)?;

for patch in &patch_bin.patches {
    // e.g. `4a47c414 Position.Anchors.Anchor = Vector2`
    println!("{:08x} {} = {:?}", patch.object_hash, patch.path, patch.kind());
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Applying an override bin

A patch only means something over the bin it patches. [`BinOverride::apply`] lays it over one,
in the order the client does, and reports what it could not apply instead of failing - which is
also what the client does with a stale path. [`BinOverride::check`] answers the same question
without touching anything, which is what to ask after a game update.

```no_run
use std::fs::File;
use ltk_meta::{Bin, BinOverride};

let mut base = Bin::from_reader(&mut File::open("uibase.bin")?)?;
let patch_bin = BinOverride::from_reader(&mut File::open("uiflipped.bin")?)?;

// Does it still fit?
let report = patch_bin.check(&base);
println!("{report}"); // "109 applied (5 inserted), 0 skipped, ..."

if report.is_clean() {
    patch_bin.apply(&mut base);
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Addressing a property by path

[`PropertyPath`](path::PropertyPath) is the language a patch record uses to name what it
overrides, and it works on any object: `Position.UIRect.Size`, `Elements[3]`,
`Lookup{"weapon"}`.

```no_run
use std::fs::File;
use ltk_meta::{path::PropertyPath, property::values, Bin};

let mut bin = Bin::from_reader(&mut File::open("uibase.bin")?)?;
let anchor = PropertyPath::new("Position.Anchors.Anchor")?;

println!("{:?}", bin.resolve(0x4a47c414_u32, &anchor)?);

// `patch` follows the client's type rule: the shape has to match, or nothing changes.
bin.patch(
    0x4a47c414_u32,
    &anchor,
    values::Vector2::new(glam::Vec2::new(0.0, 1.0)).into(),
)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Reading a `.bin` of unknown kind

The extension does not say which kind a file is, so either read it as a [`BinFile`] and
match on what came back, or ask [`BinKind::identify_from_reader`] and call the reader you
want. It leaves the magic in place, so the same reader can be handed straight on.

```no_run
use std::fs::File;
use ltk_meta::{Bin, BinFile, BinKind, BinOverride};

let mut file = File::open("unknown.bin")?;
match BinFile::from_reader(&mut file)? {
    BinFile::Prop(bin) => println!("{} objects", bin.objects.len()),
    BinFile::Override(patch_bin) => println!("{} patches", patch_bin.patches.len()),
}

let mut file = File::open("unknown.bin")?;
match BinKind::identify_from_reader(&mut file)? {
    BinKind::Prop => { let bin = Bin::from_reader(&mut file)?; }
    BinKind::Override => { let patch_bin = BinOverride::from_reader(&mut file)?; }
}
# Ok::<(), Box<dyn std::error::Error>>(())
```
*/
pub mod concrete;
pub mod path;
pub mod property;
pub use property::{Kind as PropertyKind, PropertyValueEnum, ValueSlot};

mod tree;
pub use tree::*;

mod data_override;
pub use data_override::{
    ApplyReport, BinOverride, Builder as BinOverrideBuilder, PropertyPatch, SkippedPatch,
};

mod file;
pub use file::{BinFile, BinKind};

pub mod stream;
pub use stream::{
    BinStream, BinToc, Entries, Numbering, ObjectEntry, ObjectStream, ObjectView, Objects,
    PropertyView, ValueView,
};

mod error;
pub use error::*;

pub mod traits;
