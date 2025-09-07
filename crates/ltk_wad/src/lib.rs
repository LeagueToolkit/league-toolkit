//! Wad file handling

pub mod entry;
pub mod header;
mod wad;
use binrw::NamedArgs;
pub use wad::*;

mod error;
pub use error::*;

mod file_ext;
pub use file_ext::*;

mod builder;
pub use builder::*;

use std::{
    collections::HashMap,
    io::{BufReader, Read, Seek, SeekFrom},
};

#[derive(Clone, NamedArgs)]
pub struct VersionArgs {
    pub major: u8,
    pub minor: u8,
}
