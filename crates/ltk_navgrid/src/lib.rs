//! Navigation grid reading and writing for League of Legends maps.
//!
//! The crate reads and writes three formats:
//!
//! - `.aimesh_ngrid`: the navigation grid of a map, a [`NavGrid`]. Each cell has a height,
//!   terrain [`CellFlags`] and region tags. A finer grid of sampled heights covers the same
//!   area. The crate reads versions 6 and 7 and writes version 7.
//! - `.ngrid_overlay`: rectangles of cell flags, a [`NavGridOverlay`].
//! - `.rg_overlay`: rectangles of region tags, a [`RegionOverlay`].
//!
//! All values are little-endian.
//!
//! # Example
//!
//! ```no_run
//! use ltk_navgrid::{CellFlags, NavGrid};
//! use std::io::BufReader;
//! use std::fs::File;
//!
//! let mut file = BufReader::new(File::open("aipath.aimesh_ngrid")?);
//! let grid = NavGrid::from_reader(&mut file)?;
//!
//! let walls = grid.cells().iter().filter(|c| c.flags.contains(CellFlags::WALL)).count();
//! println!("{} x {} cells, {walls} walls", grid.x_cell_count(), grid.y_cell_count());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod error;
pub use error::*;

mod flags;
pub use flags::*;

mod grid;
pub use grid::*;

mod overlay;
pub use overlay::*;

/// Result type alias for this crate.
pub type Result<T, E = ParseError> = std::result::Result<T, E>;
