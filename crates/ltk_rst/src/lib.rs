//! Reading and writing League of Legends RST (Riot String Table) files.
//!
//! RST files are localisation tables that map xxHash-based keys to UTF-8
//! strings.  They are typically found at `DATA/Menu/*.stringtable` inside WAD
//! archives.
//!
//! # Format variations
//!
//! Three independent things changed over the format's history, and only the
//! first is encoded in the file:
//!
//! - The **version byte** (2–5) controls the structural framing (a V2-only
//!   font-config block, and an encryption-flag byte present in V2–V4).
//! - The **hash/offset split** (`hash_bits`) is 40 for V2/V3 and 39 *or* 38 for
//!   V4/V5 — Riot moved to 38-bit in game patch 15.2.  The file doesn't record
//!   which, so [`Stringtable::from_reader`] assumes the latest (38-bit); for an
//!   older V4/V5 file, opt into detection with [`Stringtable::reader`]`.`[`detect_hash_bits`](RstReader::detect_hash_bits)
//!   or pin it with [`RstReader::hash_bits`].
//! - The **hash algorithm** is xxHash64 before game patch 14.15 and XXH3 since.
//!   This is never needed to *read* a table (stored hashes are read directly);
//!   it only matters when hashing a key string for lookup or insertion.
//!
//! All three are captured in [`RstFormat`]; use
//! [`RstFormat::for_patch`] to build one from a game patch number, or pin the
//! reader explicitly via [`Stringtable::reader`]'s
//! [`hash_bits`](RstReader::hash_bits) / [`hash_algo`](RstReader::hash_algo).
//!
//! # Reading
//!
//! ```no_run
//! use std::fs::File;
//! use ltk_rst::Stringtable;
//!
//! let mut file = File::open("en_us/bootstrap.stringtable")?;
//! let table = Stringtable::from_reader(&mut file)?;
//!
//! for (hash, text) in table.iter() {
//!     println!("{:#018x} = {text}", *hash);
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Editing and writing
//!
//! ```no_run
//! use std::fs::File;
//! use ltk_rst::Stringtable;
//!
//! let mut table = Stringtable::new();
//! table.insert_str("game_client_quit", "Quit");
//!
//! let mut out = File::create("out.stringtable")?;
//! table.to_writer(&mut out)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod error;
mod hash;
mod rst;
mod version;

pub use error::*;
pub use hash::*;
pub use rst::*;
pub use version::*;
