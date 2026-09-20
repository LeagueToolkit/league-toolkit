use glam::{vec2, vec3, vec4, Vec2, Vec3, Vec4};
use half::f16;
use std::marker::PhantomData;

use super::vertex::{ComponentType, ElementFormat, VertexBuffer, VertexElement};

/// Reads one vertex element out of a packed vertex buffer.
///
/// Implemented for the types the League vertex formats decode to: [`f32`], [`Vec2`],
/// [`Vec3`], [`Vec4`] and `[u8; 4]`.
///
/// A type decodes an element whose format carries at least as many components as the type
/// needs, in the same component kind. [`Vec3`] reads a `XYZ_Float32` normal and the `xyz` of
/// a `XYZW_Float16` one; `[u8; 4]` reads a packed colour or a set of blend indices. A type
/// never reads an element with fewer components than it needs, and never reads a byte format
/// as floats.
pub trait Format {
    /// The value one element decodes to.
    type Item;

    /// Whether this type decodes an element packed in `format`.
    #[must_use]
    fn decodes(format: ElementFormat) -> bool;

    /// Reads the element of vertex `index`, which begins `element_offset` bytes into it.
    ///
    /// `format` is the element's own packing, which decides how many bytes each component
    /// takes and how it is widened.
    ///
    /// # Panics
    /// Panics if the element does not lie inside the buffer.
    #[must_use]
    fn read(
        buffer: &VertexBuffer,
        format: ElementFormat,
        index: usize,
        element_offset: usize,
    ) -> Self::Item;
}

/// Get the offset of a single vertex element for a single vertex in a vertex buffer.
fn offset(buffer: &VertexBuffer, index: usize, element_offset: usize) -> usize {
    buffer.stride() * index + element_offset
}

/// Reads the first `N` components of an element as [`f32`], widening a half or a byte.
///
/// # Panics
/// Panics if the element does not lie inside the buffer.
fn read_floats<const N: usize>(
    buffer: &VertexBuffer,
    format: ElementFormat,
    index: usize,
    element_offset: usize,
) -> [f32; N] {
    let base = offset(buffer, index, element_offset);
    let bytes = buffer.as_bytes();

    match format.component_type() {
        ComponentType::Float32 => std::array::from_fn(|i| {
            let at = base + i * 4;
            f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
        }),
        ComponentType::Float16 => std::array::from_fn(|i| {
            let at = base + i * 2;
            f16::from_le_bytes([bytes[at], bytes[at + 1]]).to_f32()
        }),
        ComponentType::UInt8 => std::array::from_fn(|i| f32::from(bytes[base + i])),
    }
}

/// Whether `format` holds at least `n` float components.
fn decodes_floats(format: ElementFormat, n: usize) -> bool {
    format.component_type().is_float() && format.component_count() >= n
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
    /// Creates an accessor over an element `T` decodes.
    ///
    /// [`VertexBuffer::accessor`] checks the element's format against
    /// [`Format::decodes`] before calling this.
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
        T::read(self.buffer, self.element.format, index, self.element_off)
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
}

impl Format for f32 {
    type Item = f32;

    fn decodes(format: ElementFormat) -> bool {
        decodes_floats(format, 1)
    }

    fn read(buffer: &VertexBuffer, format: ElementFormat, index: usize, element_off: usize) -> f32 {
        let [x] = read_floats::<1>(buffer, format, index, element_off);
        x
    }
}

impl Format for Vec2 {
    type Item = Vec2;

    fn decodes(format: ElementFormat) -> bool {
        decodes_floats(format, 2)
    }

    fn read(
        buffer: &VertexBuffer,
        format: ElementFormat,
        index: usize,
        element_off: usize,
    ) -> Vec2 {
        let [x, y] = read_floats::<2>(buffer, format, index, element_off);
        vec2(x, y)
    }
}

impl Format for Vec3 {
    type Item = Vec3;

    fn decodes(format: ElementFormat) -> bool {
        decodes_floats(format, 3)
    }

    fn read(
        buffer: &VertexBuffer,
        format: ElementFormat,
        index: usize,
        element_off: usize,
    ) -> Vec3 {
        let [x, y, z] = read_floats::<3>(buffer, format, index, element_off);
        vec3(x, y, z)
    }
}

impl Format for Vec4 {
    type Item = Vec4;

    fn decodes(format: ElementFormat) -> bool {
        decodes_floats(format, 4)
    }

    fn read(
        buffer: &VertexBuffer,
        format: ElementFormat,
        index: usize,
        element_off: usize,
    ) -> Vec4 {
        let [x, y, z, w] = read_floats::<4>(buffer, format, index, element_off);
        vec4(x, y, z, w)
    }
}

impl Format for [u8; 4] {
    type Item = [u8; 4];

    fn decodes(format: ElementFormat) -> bool {
        format.component_type() == ComponentType::UInt8 && format.component_count() >= 4
    }

    fn read(
        buffer: &VertexBuffer,
        _format: ElementFormat,
        index: usize,
        element_off: usize,
    ) -> [u8; 4] {
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
        let item = T::read(
            self.view.buffer,
            self.view.element.format,
            self.counter,
            self.view.element_off,
        );
        self.counter += 1;
        Some(item)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.view.buffer.count().saturating_sub(self.counter);
        (remaining, Some(remaining))
    }
}

impl<T: Format> ExactSizeIterator for Iter<'_, T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem::vertex::{
        ElementName, VertexBufferDescription, VertexBufferUsage, VertexElement,
    };

    /// A vertex buffer holding `elements`, over the bytes given.
    fn buffer(elements: Vec<VertexElement>, bytes: Vec<u8>) -> VertexBuffer {
        VertexBuffer::new(
            VertexBufferDescription::new(VertexBufferUsage::Static, elements),
            bytes,
        )
    }

    fn half_bytes(values: &[f32]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|&v| f16::from_f32(v).to_le_bytes())
            .collect()
    }

    #[test]
    fn a_half_uv_decodes_to_f32() {
        let uv = VertexElement::new(ElementName::Texcoord0, ElementFormat::XY_Float16);
        let buffer = buffer(vec![uv], half_bytes(&[0.25, 0.75]));

        let accessor = buffer
            .accessor::<Vec2>(ElementName::Texcoord0)
            .expect("Vec2 decodes a two component half UV");
        assert_eq!(accessor.get(0), vec2(0.25, 0.75));
    }

    #[test]
    fn a_half_normal_decodes_to_vec3_dropping_w() {
        let normal = VertexElement::new(ElementName::Normal, ElementFormat::XYZW_Float16);
        let buffer = buffer(vec![normal], half_bytes(&[1.0, 0.0, 0.0, 0.5]));

        let accessor = buffer
            .accessor::<Vec3>(ElementName::Normal)
            .expect("Vec3 decodes the xyz of a four component half normal");
        assert_eq!(accessor.get(0), vec3(1.0, 0.0, 0.0));

        let accessor = buffer
            .accessor::<Vec4>(ElementName::Normal)
            .expect("Vec4 decodes all four components");
        assert_eq!(accessor.get(0), vec4(1.0, 0.0, 0.0, 0.5));
    }

    #[test]
    fn a_half_element_is_read_at_its_own_stride() {
        // Two vertices of one half UV each: 4 bytes per vertex, not 8.
        let uv = VertexElement::new(ElementName::Texcoord0, ElementFormat::XY_Float16);
        let buffer = buffer(vec![uv], half_bytes(&[0.0, 1.0, 2.0, 3.0]));

        assert_eq!(buffer.stride(), 4);
        assert_eq!(buffer.count(), 2);

        let accessor = buffer.accessor::<Vec2>(ElementName::Texcoord0).unwrap();
        assert_eq!(
            accessor.iter().collect::<Vec<_>>(),
            vec![vec2(0.0, 1.0), vec2(2.0, 3.0)]
        );
    }

    #[test]
    fn a_type_wider_than_the_element_has_no_accessor() {
        let uv = VertexElement::new(ElementName::Texcoord0, ElementFormat::XY_Float16);
        let buffer = buffer(vec![uv], half_bytes(&[0.25, 0.75]));

        // Reading four bytes of half UV as a Vec3 would run off the vertex.
        assert!(buffer.accessor::<Vec3>(ElementName::Texcoord0).is_none());
        assert!(buffer.accessor::<Vec4>(ElementName::Texcoord0).is_none());
        // And a UV is not a packed colour.
        assert!(buffer.accessor::<[u8; 4]>(ElementName::Texcoord0).is_none());
    }

    #[test]
    fn a_packed_colour_is_not_read_as_floats() {
        let colour = VertexElement::new(ElementName::PrimaryColor, ElementFormat::BGRA_Packed8888);
        let buffer = buffer(vec![colour], vec![1, 2, 3, 4]);

        assert!(buffer.accessor::<Vec4>(ElementName::PrimaryColor).is_none());
        assert_eq!(
            buffer
                .accessor::<[u8; 4]>(ElementName::PrimaryColor)
                .unwrap()
                .get(0),
            [1, 2, 3, 4]
        );
    }

    #[test]
    fn a_float32_element_still_decodes() {
        let bytes: Vec<u8> = [1.0f32, 2.0, 3.0]
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        let buffer = buffer(vec![VertexElement::POSITION], bytes);

        let accessor = buffer.accessor::<Vec3>(ElementName::Position).unwrap();
        assert_eq!(accessor.get(0), vec3(1.0, 2.0, 3.0));
    }
}
