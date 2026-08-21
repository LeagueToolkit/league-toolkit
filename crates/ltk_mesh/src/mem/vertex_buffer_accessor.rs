use glam::{vec2, vec3, vec4, Vec2, Vec3, Vec4};
use std::marker::PhantomData;

use super::vertex::{VertexBuffer, VertexElement};

/// Reads one vertex element out of a packed vertex buffer.
///
/// Implemented for the types the League vertex formats decode to: [`f32`], [`Vec2`],
/// [`Vec3`], [`Vec4`] and `[u8; 4]`.
pub trait Format {
    /// The value one element decodes to.
    type Item;

    /// Reads the element of vertex `index`, which begins `element_offset` bytes into it.
    ///
    /// # Panics
    /// Panics if the element does not lie inside the buffer.
    #[must_use]
    fn read(buffer: &VertexBuffer, index: usize, element_offset: usize) -> Self::Item;
}

/// Get the offset of a single vertex element for a single vertex in a vertex buffer.
fn offset(buffer: &VertexBuffer, index: usize, element_offset: usize) -> usize {
    buffer.stride() * index + element_offset
}

/// A view over all vertices of a single [`VertexElement`] in a [`VertexBuffer`].
///
/// Resolving one costs a lookup, so build it once and reuse it across a whole pass rather
/// than per vertex. Created by [`VertexBuffer::accessor`].
pub struct VertexBufferAccessor<'a, T: Format> {
    buffer: &'a VertexBuffer,
    element: VertexElement,
    element_off: usize,

    _t: PhantomData<T>,
}

impl<T: Format> std::fmt::Debug for VertexBufferAccessor<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VertexBufferAccessor")
            .field("element", &self.element)
            .field("offset", &self.element_off)
            .field("len", &self.len())
            .finish()
    }
}

impl<'a, T: Format> VertexBufferAccessor<'a, T> {
    /// Creates a new VertexBufferAccessor. The type of element is **not** checked, so the caller must ensure that the element format matches the format of the accessor.
    pub(super) fn new(
        element: VertexElement,
        element_off: usize,
        buffer: &'a VertexBuffer,
    ) -> VertexBufferAccessor<'a, T> {
        VertexBufferAccessor {
            buffer,
            element,
            element_off,
            _t: PhantomData,
        }
    }

    /// The element this accessor views, including its format.
    #[inline(always)]
    #[must_use]
    pub fn element(&self) -> VertexElement {
        self.element
    }

    /// Iterates the element over **every** vertex in the buffer.
    ///
    /// To walk indexed data, or only the vertices one range owns, use
    /// [`VertexBufferAccessor::get`] instead - `iter().nth(i)` is O(i).
    #[inline(always)]
    #[must_use]
    pub fn iter(&'a self) -> Iter<'a, T> {
        Iter {
            view: self,
            counter: 0,
        }
    }

    /// Reads the element of a single vertex, by its index in the buffer.
    ///
    /// This is the random access an indexed mesh walk needs. Resolve the accessor once and
    /// reuse it - building one costs a lookup, `get` costs an offset and a load.
    ///
    /// # Panics
    /// Panics if `index` is out of bounds.
    #[inline]
    #[must_use]
    pub fn get(&self, index: usize) -> T::Item {
        T::read(self.buffer, index, self.element_off)
    }

    /// The number of vertices this accessor spans.
    #[inline(always)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.buffer.count()
    }

    /// Whether the buffer this views holds no vertices.
    #[inline(always)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    // TODO (alan): impl the rest of the ElementFormat's
}

// TODO(alan): figure out endianness (again)

impl Format for f32 {
    type Item = f32;
    fn read(buffer: &VertexBuffer, index: usize, element_off: usize) -> f32 {
        let offset = offset(buffer, index, element_off);
        let buf = buffer.as_bytes();
        f32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap())
    }
}

impl Format for Vec2 {
    type Item = Vec2;
    fn read(buffer: &VertexBuffer, index: usize, element_off: usize) -> Vec2 {
        let offset = offset(buffer, index, element_off);
        let buf = buffer.as_bytes();
        let x = f32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap());
        let y = f32::from_le_bytes(buf[offset + 4..offset + 8].try_into().unwrap());
        vec2(x, y)
    }
}

impl Format for Vec3 {
    type Item = Vec3;
    fn read(buffer: &VertexBuffer, index: usize, element_off: usize) -> Vec3 {
        let offset = offset(buffer, index, element_off);
        let buf = buffer.as_bytes();
        let x = f32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap());
        let y = f32::from_le_bytes(buf[offset + 4..offset + 8].try_into().unwrap());
        let z = f32::from_le_bytes(buf[offset + 8..offset + 12].try_into().unwrap());
        vec3(x, y, z)
    }
}

impl Format for Vec4 {
    type Item = Vec4;
    fn read(buffer: &VertexBuffer, index: usize, element_off: usize) -> Vec4 {
        let offset = offset(buffer, index, element_off);
        let buf = buffer.as_bytes();
        let x = f32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap());
        let y = f32::from_le_bytes(buf[offset + 4..offset + 8].try_into().unwrap());
        let z = f32::from_le_bytes(buf[offset + 8..offset + 12].try_into().unwrap());
        let w = f32::from_le_bytes(buf[offset + 12..offset + 16].try_into().unwrap());
        vec4(x, y, z, w)
    }
}

impl Format for [u8; 4] {
    type Item = [u8; 4];
    fn read(buffer: &VertexBuffer, index: usize, element_off: usize) -> [u8; 4] {
        let offset = offset(buffer, index, element_off);
        let buf = buffer.as_bytes();
        [
            buf[offset],
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
        ]
    }
}

impl<'a, T: Format> IntoIterator for &'a VertexBufferAccessor<'a, T> {
    type Item = T::Item;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator over one element of every vertex, created by [`VertexBufferAccessor::iter`].
pub struct Iter<'a, T: Format> {
    view: &'a VertexBufferAccessor<'a, T>,
    counter: usize,
}

impl<T: Format> std::fmt::Debug for Iter<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Iter")
            .field("element", &self.view.element)
            .field("position", &self.counter)
            .field("len", &self.view.len())
            .finish()
    }
}

impl<T: Format> Iterator for Iter<'_, T> {
    type Item = T::Item;

    fn next(&mut self) -> Option<Self::Item> {
        if self.counter >= self.view.buffer.count() {
            return None;
        }
        let item = T::read(self.view.buffer, self.counter, self.view.element_off);
        self.counter += 1;
        Some(item)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.view.buffer.count().saturating_sub(self.counter);
        (remaining, Some(remaining))
    }
}

impl<T: Format> ExactSizeIterator for Iter<'_, T> {}
