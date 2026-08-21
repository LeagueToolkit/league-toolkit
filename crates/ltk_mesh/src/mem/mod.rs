//! Index and vertex buffers shared by every mesh format.
//!
//! These are the GPU-shaped containers the mesh types are built from: [`IndexBuffer`] holds
//! packed indices of a fixed width, [`VertexBuffer`] holds interleaved vertex data plus the
//! [`VertexBufferDescription`] that says how to decode it. Both hand out their raw bytes, so
//! a mesh can be uploaded without a conversion pass.
//!
//! Read a single attribute out of a vertex buffer with [`VertexBuffer::accessor`], which
//! yields a [`VertexBufferAccessor`] view over one element of every vertex.
pub mod index;

#[doc(inline)]
pub use index::IndexBuffer;

mod vertex_buffer;
mod vertex_buffer_accessor;
mod vertex_buffer_description;
mod vertex_element;

pub mod vertex {
    //! Vertex buffer types: layouts, elements and element accessors.
    #[doc(inline)]
    pub use super::vertex_buffer::{VertexBuffer, VertexBufferElementDescriptor};
    #[doc(inline)]
    pub use super::vertex_buffer_accessor::{Format, Iter, VertexBufferAccessor};
    #[doc(inline)]
    pub use super::vertex_buffer_description::{
        get_element_flags, VertexBufferDescription, VertexBufferElementFlags, VertexBufferUsage,
    };
    #[doc(inline)]
    pub use super::vertex_element::{ElementFormat, ElementName, VertexElement};
}

pub use vertex::{
    VertexBuffer, VertexBufferAccessor, VertexBufferDescription, VertexBufferElementDescriptor,
    VertexBufferElementFlags, VertexBufferUsage, VertexElement,
};
