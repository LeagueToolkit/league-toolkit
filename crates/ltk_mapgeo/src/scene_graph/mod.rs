//! Spatial scene graph structures for environment assets.
//!
//! The scene graph provides spatial partitioning via bucketed geometry,
//! enabling efficient visibility queries, culling, and collision detection.

mod bucketed_geometry;
mod bucket;

pub use bucketed_geometry::*;
pub use bucket::*;

