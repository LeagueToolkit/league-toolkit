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
//!   ([`BinStream::object`]) costs one harvest at most.
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
//! The [`wire`] module underneath owns the byte-level layout every layer shares: fixed widths,
//! skip distances and wire-header shapes, one implementation for the whole crate.

mod cursor;
pub use cursor::{Entries, ObjectStream, Objects};

mod prop;
pub use prop::BinStream;

mod toc;
pub use toc::{BinToc, ObjectEntry};

pub mod wire;

#[cfg(test)]
mod tests;
