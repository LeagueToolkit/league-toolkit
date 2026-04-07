//! Reading and writing League of Legends RST (Riot String Table) files.
//!
//! RST files are localisation tables that map XXHash64-based keys to UTF-8
//! strings.  They are typically found at `DATA/Menu/*.stringtable` or
//! `DATA/Menu/fontconfig_*.txt` inside WAD archives.
//!
//! # Reading
//!
//! ```no_run
//! use std::fs::File;
//! use ltk_rst::Stringtable;
//!
//! let mut file = File::open("fontconfig_en_us.stringtable")?;
//! let table = Stringtable::from_rst_reader(&mut file)?;
//!
//! for (hash, text) in &table.entries {
//!     println!("{hash:#018x} = {text}");
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Writing
//!
//! ```no_run
//! use std::fs::File;
//! use ltk_rst::Stringtable;
//!
//! let mut table = Stringtable::new();
//! table.insert_str("game_client_quit", "Quit");
//!
//! let mut out = File::create("out.stringtable")?;
//! table.to_rst_writer(&mut out)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Hashing keys manually
//!
//! ```
//! use ltk_rst::{RstHashType, compute_hash};
//!
//! let hash = compute_hash("game_client_quit", RstHashType::Simple);
//! println!("{hash:#018x}");
//! ```

mod error;
mod hash;
mod rst;
mod version;

pub use error::*;
pub use hash::*;
pub use rst::*;
pub use version::*;
