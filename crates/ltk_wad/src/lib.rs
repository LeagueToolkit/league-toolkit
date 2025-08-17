//! Wad file handling

pub mod entry;
pub mod header;
mod wad;
pub use wad::*;

mod error;
mod file_ext;

pub use error::*;
pub use file_ext::*;

use std::{
    collections::HashMap,
    io::{BufReader, Read, Seek, SeekFrom},
};
