//! Reads, edits and writes the binary ini files of League of Legends: `.inibin`, `.troybin` and
//! `.cfgbin`.
//!
//! The three extensions have the same format. An `.inibin` file contains game data, a `.troybin`
//! file contains a particle system and a `.cfgbin` file contains configuration. The ini compiler
//! of the game builds each file from an ini text file. The client looks up a value by the hash of
//! its section name and property name.
//!
//! # Reading
//!
//! [`Inibin::section`] returns a view that accesses the values of one section by property name.
//! A typed getter returns the value that the client reads.
//!
//! ```no_run
//! use std::fs::File;
//! use ltk_ini::Inibin;
//!
//! let troybin = Inibin::from_reader(&mut File::open("flash.troybin")?)?;
//!
//! if let Some(emitter) = troybin.section("System").get_str("GroupPart0") {
//!     let emitter_values = troybin.section(emitter);
//!     let life = emitter_values.get_f32("p-life").unwrap_or(1.0);
//!     let texture = emitter_values.get_str("p-texture");
//!     println!("{emitter}: p-life = {life}, p-texture = {texture:?}");
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Editing and writing
//!
//! ```no_run
//! use std::fs::File;
//! use ltk_ini::Inibin;
//!
//! let mut troybin = Inibin::from_reader(&mut File::open("flash.troybin")?)?;
//!
//! let mut flash = troybin.section_mut("Flash");
//! flash.insert("p-life", 2.5);
//! flash.insert("p-texture", "ASSETS/Shared/Particles/flash.dds");
//! flash.remove("p-trailmode");
//!
//! troybin.to_writer(&mut File::create("flash.troybin")?)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Keys
//!
//! An inibin file stores the hash of each pair of section name and property name. The file does
//! not store the names, and the sections and properties of a file cannot be listed.
//! [`IniHash::new`] hashes a pair of names. The methods of [`Inibin`] take a key. Use them if only
//! the hash is known.
//!
//! # Values
//!
//! [`Value`] has one variant for each of the 14 storage types of the format. The ini compiler
//! stores a value as a string, or as a number in the smallest storage type that can store it. One
//! property has different storage types in different files. The typed getters and the `to_*`
//! methods of [`Value`] convert from every storage type that the client converts from. The
//! `compact_*` constructors of [`Value`] select the smallest storage type.
//!
//! # Ini text
//!
//! [`Inibin::from_ini`] reads ini text into an `Inibin`, with every value a string. The names of
//! the text give the keys. By default a line that starts with `'`, `*`, `#`, `//` or `` ` `` is a
//! property, as the ini compiler of old builds reads it ([`IniTextOptions`]).

mod error;
mod inibin;
mod kind;
mod scan;
mod section;
mod text;
mod value;

pub use error::{Error, Result};
pub use inibin::Inibin;
pub use kind::ValueKind;
pub use ltk_hash::IniHash;
pub use section::{Section, SectionMut};
pub use text::{IniTextOptions, ADDITIONAL_COMMENT_TOKENS};
pub use value::Value;
