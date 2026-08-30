//! Streaming access to `PROP` bins: mount a file, read only what is asked for.
//!
//! [`Bin::from_reader`](crate::Bin::from_reader) parses every property of every object into a
//! tree. That is the wrong shape for harvesting object hashes across thousands of files, or for
//! reading a header without touching the body. The client's own loader is a one-pass streaming
//! reader that seeks past whatever it will not parse, and this module takes that model:
//!
//! - [`BinStream::mount`] reads the header, dependencies and class-hash table, then stops.
//! - [`BinStream::entries`] sweeps the object table, yielding one [`ObjectEntry`] descriptor
//!   per object and skipping every body by its size field.
//! - [`BinStream::toc`] caches the sweep as a [`BinToc`], so random access by path hash
//!   ([`BinStream::object`]) costs one harvest at most, and
//!   [`BinStream::objects_batch`] resolves a whole set of hashes on one forward schedule.
//! - [`ObjectStream::view`] buffers one object's declared byte range and views it zero-copy:
//!   [`ObjectView`] iterates and looks up properties, and [`ValueView`] descends into a value
//!   to any depth without materializing anything.
//! - [`ObjectStream::read`] and [`BinStream::into_bin`] are the owned way out, and
//!   [`BinStream::cached_object`] is the same through an installed [`ObjectCache`].
//!
//! ```no_run
//! use std::fs::File;
//! use ltk_meta::concrete::BinStream;
//!
//! let mut stream = BinStream::mount(File::open("data.bin")?)?;
//!
//! for entry in stream.entries() {
//!     let entry = entry?;
//!     println!("{:08x}: {:08x}", entry.path_hash, entry.class_hash);
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Underneath, an internal layout core owns the byte-level knowledge every layer shares:
//! where a value starts, how far it runs, what its header declares, and how to decode a leaf.
//! One cursor carries all of it, along with the [`Numbering`] its bytes were written under.
//! The views and the owned decode are two renderers over that one cursor, and
//! [`Bin::from_reader`](crate::Bin::from_reader) is [`BinStream::mount`] plus
//! [`BinStream::into_bin`] — so the crate has one parser, and the eager tree and the streaming
//! surface cannot drift.

mod batch;
pub use batch::BatchObjects;

mod cache;
pub use cache::{LruObjectCache, NoCache, ObjectCache};

mod cursor;
pub use cursor::{Entries, ObjectStream, Objects};

pub(crate) mod owned;

mod prop;
pub use prop::BinStream;

mod toc;
pub use toc::{BinToc, ObjectEntry};

mod view;
pub use view::{
    ContainerItems, ContainerView, MapEntries, MapView, ObjectView, OptionalView, Properties,
    PropertyView, StructView, ValueView,
};

pub(crate) mod layout;
pub use layout::Numbering;

#[cfg(test)]
mod tests;
