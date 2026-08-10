#![warn(missing_docs)]
#![warn(clippy::missing_errors_doc, clippy::missing_panics_doc)]
//! Skinned & static meshes
//!
//! [`SkinnedMesh`] parses `.skn` (character meshes, skinning data included) and
//! [`StaticMesh`] parses `.scb`/`.sco` (environment geometry).
//!
//! ```no_run
//! use std::{fs::File, io::BufReader};
//!
//! use glam::Vec3;
//! use ltk_mesh::{mem::vertex::ElementName, SkinnedMesh};
//!
//! // Buffer the file - the header and submesh table are read in small pieces.
//! let mesh = SkinnedMesh::from_reader(&mut BufReader::new(File::open("champion.skn")?))?;
//!
//! // Resolve accessors once and reuse them; each one costs a lookup.
//! let positions = mesh
//!     .vertex_buffer()
//!     .accessor::<Vec3>(ElementName::Position)
//!     .expect("every .skn carries positions");
//!
//! for range in mesh.ranges() {
//!     // range_indices() yields absolute indices into the shared vertex buffer.
//!     let corners = mesh.range_indices(range).map(|i| positions.get(i as usize));
//!     println!("{}: {} vertices", range.material, corners.count());
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! See the [`SkinnedMesh`] docs for the indexing rules and the GPU upload path, and
//! `examples/skinned_mesh.rs` for a complete walk over a real file.
pub mod error;
pub mod mem;

// Private, so every mesh type has exactly one public path: the crate root.
mod skinned;
mod r#static;

use error::ParseError;

#[doc(inline)]
pub use r#static::{StaticMesh, StaticMeshFace, StaticMeshFlags, SCB_MAGIC};

#[doc(inline)]
pub use skinned::{
    RangeIndices, SkinnedMesh, SkinnedMeshFlags, SkinnedMeshRange, SkinnedMeshVertexType,
    END_TAB_SIZE, MAX_VERTEX_COUNT, SKN_MAGIC,
};

/// Result of any mesh read or write.
pub type Result<T> = core::result::Result<T, ParseError>;
