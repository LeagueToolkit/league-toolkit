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

```
use ltk_meta::{Bin, BinObject};
use ltk_meta::property::{values, NoMeta};

// Using the builder pattern
let tree = Bin::builder()
    .dependency("common.bin")
    .object(
        BinObject::<NoMeta>::builder(0x12345678, 0xABCDEF00)
            .property(0x1111, values::I32::new(42))
            .property(0x2222, values::String::from("hello"))
            .build()
    )
    .build();

// Or using the simple constructor
let tree = Bin::new(
    [BinObject::<NoMeta>::new(0x1234, 0x5678)],
    ["dependency.bin"],
);
```

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
pub mod path;
pub mod property;
pub use property::{Kind as PropertyKind, PropertyValueEnum};

mod tree;
pub use tree::*;

mod data_override;
pub use data_override::{BinOverride, Builder as BinOverrideBuilder, PropertyPatch};

mod file;
pub use file::{BinFile, BinKind};

mod error;
pub use error::*;

pub mod traits;
