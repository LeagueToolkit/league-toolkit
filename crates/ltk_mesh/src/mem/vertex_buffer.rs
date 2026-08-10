use std::collections::BTreeMap;
use std::fmt::Debug;

use super::vertex::{
    ElementName, Format, VertexBufferAccessor, VertexBufferDescription, VertexElement,
};

/// Where one [`VertexElement`] sits within a vertex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VertexBufferElementDescriptor {
    element: VertexElement,
    offset: isize,
}

impl VertexBufferElementDescriptor {
    /// Pairs an element with its byte offset into a vertex.
    #[must_use]
    pub fn new(element: VertexElement, offset: isize) -> Self {
        Self { element, offset }
    }

    /// The element being described.
    #[must_use]
    pub fn element(&self) -> VertexElement {
        self.element
    }

    /// The element's byte offset into a vertex.
    #[must_use]
    pub fn offset(&self) -> isize {
        self.offset
    }
}

/// Interleaved vertex data plus the layout describing it.
///
/// Each vertex element is a single attribute - position, normal, colour and so on - stored
/// inline with the others rather than in its own stream. Read one out with
/// [`VertexBuffer::accessor`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VertexBuffer {
    description: VertexBufferDescription,
    elements: BTreeMap<ElementName, VertexBufferElementDescriptor>,

    stride: usize,
    count: usize,

    buffer: Vec<u8>,
}

impl VertexBuffer {
    /// Creates a vertex buffer from packed bytes and the layout describing them.
    ///
    /// An empty `buffer` is accepted and yields a buffer of zero vertices.
    ///
    /// # Panics
    /// Panics if `description` has no elements or names the same [`ElementName`] twice, or if
    /// `buffer` is not a whole number of vertices long. All three are layout mistakes on the
    /// caller's side, not data the mesh formats can express.
    #[must_use]
    pub fn new(description: VertexBufferDescription, buffer: Vec<u8>) -> Self {
        let mut element_descriptors = BTreeMap::new();
        let mut off = 0;
        for &e in description.elements() {
            assert!(
                !element_descriptors.contains_key(&e.name),
                "vertex buffer has a duplicate element: {:?} appears twice",
                e.name
            );
            element_descriptors.insert(e.name, VertexBufferElementDescriptor::new(e, off as isize));
            off += e.size()
        }
        // off collects the sizes of all the elements, which also happens to be the stride
        let stride = off;

        assert!(stride > 0, "vertex buffer layout has no elements");
        assert!(
            buffer.len().is_multiple_of(stride),
            "vertex buffer size must be a multiple of its stride: got {} bytes, stride is {stride}",
            buffer.len()
        );

        Self {
            description,
            elements: element_descriptors,
            stride,
            count: buffer.len() / stride,
            buffer,
        }
    }

    /// A view over one element of every vertex, or [`None`] if this layout has no such
    /// element.
    ///
    /// `T` must match the element's [`ElementFormat`](super::vertex::ElementFormat) - `Vec3`
    /// for a position or normal, `Vec2` for a UV, `Vec4` for blend weights or a tangent,
    /// `[u8; 4]` for blend indices or a packed colour. It is **not** checked.
    ///
    /// # Examples
    /// ```
    /// use glam::Vec3;
    /// use ltk_mesh::{
    ///     mem::vertex::ElementName, mem::VertexBuffer, mem::VertexBufferDescription,
    ///     SkinnedMeshVertexType,
    /// };
    ///
    /// let vertices = VertexBuffer::new(
    ///     VertexBufferDescription::from(SkinnedMeshVertexType::Basic),
    ///     vec![0; 52 * 3],
    /// );
    ///
    /// assert!(vertices.accessor::<Vec3>(ElementName::Position).is_some());
    /// // The 52 byte layout carries no colour.
    /// assert!(vertices.accessor::<[u8; 4]>(ElementName::PrimaryColor).is_none());
    /// ```
    #[must_use]
    pub fn accessor<T: Format>(
        &self,
        element_name: ElementName,
    ) -> Option<VertexBufferAccessor<'_, T>> {
        self.elements
            .get(&element_name)
            .map(|desc| VertexBufferAccessor::<T>::new(desc.element, desc.offset as usize, self))
    }

    /// The layout of a single vertex.
    #[must_use]
    pub fn description(&self) -> &VertexBufferDescription {
        &self.description
    }

    /// Every element in the layout, keyed by name, with its offset into a vertex.
    #[must_use]
    pub fn elements(&self) -> &BTreeMap<ElementName, VertexBufferElementDescriptor> {
        &self.elements
    }

    /// The size in bytes of a single vertex, i.e. all its elements combined.
    #[must_use]
    pub fn stride(&self) -> usize {
        self.stride
    }

    /// The number of vertices in the buffer.
    #[must_use]
    pub fn count(&self) -> usize {
        self.count
    }

    /// Whether the buffer holds no vertices.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The raw interleaved bytes, ready to upload without a copy.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.buffer
    }

    /// Takes ownership of the underlying bytes.
    #[must_use]
    pub fn into_inner(self) -> Vec<u8> {
        self.buffer
    }
}
