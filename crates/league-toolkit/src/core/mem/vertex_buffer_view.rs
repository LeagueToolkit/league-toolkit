use std::marker::PhantomData;

use log::debug;
use vecmath::{Vector2, Vector3, Vector4};

use crate::core::mem::ElementFormat;

use super::{VertexBuffer, VertexElement};

macro_rules! repack_self {
    ($self:ident) => {
        VertexBufferView {
            buffer: $self.buffer,
            element: $self.element,
            element_off: $self.element_off,
            _t: PhantomData,
        }
    };
}

/// A view of a single VertexElement over a VertexBuffer
pub struct VertexBufferView<'a, T> {
    buffer: &'a VertexBuffer,
    element: VertexElement,
    element_off: usize,

    _t: PhantomData<T>,
}
impl<'a, T> VertexBufferView<'a, T> {
    pub(super) fn new(
        element: VertexElement,
        element_off: usize,
        buffer: &'a VertexBuffer,
    ) -> VertexBufferView<'a, ()> {
        VertexBufferView {
            buffer,
            element,
            element_off,
            _t: PhantomData,
        }
    }
    fn offset(&self, index: usize) -> usize {
        self.buffer.stride() * index + self.element_off
    }

    pub fn iter(&'a self) -> VertexBufferViewIter<'a, T> {
        VertexBufferViewIter {
            view: self,
            counter: 0,
        }
    }

    pub fn as_f32(self) -> VertexBufferView<'a, f32> {
        assert_eq!(self.element.format, ElementFormat::X_Float32);
        repack_self!(self)
    }
    pub fn as_vec2(self) -> VertexBufferView<'a, Vector2<f32>> {
        assert_eq!(self.element.format, ElementFormat::XY_Float32);
        repack_self!(self)
    }
    pub fn as_vec3(self) -> VertexBufferView<'a, Vector3<f32>> {
        assert_eq!(self.element.format, ElementFormat::XYZ_Float32);
        repack_self!(self)
    }
    pub fn as_vec4(self) -> VertexBufferView<'a, Vector4<f32>> {
        assert_eq!(self.element.format, ElementFormat::XYZW_Float32);
        repack_self!(self)
    }
    // TODO (alan): impl the rest of the ElementFormat's
}

// TODO(alan): figure out endianness (again)

impl<'a> VertexBufferView<'a, f32> {
    pub fn get(&self, index: usize) -> f32 {
        let offset = self.offset(index);
        let buf = self.buffer.buffer();
        f32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap())
    }
}

impl<'a> VertexBufferView<'a, Vector2<f32>> {
    pub fn get(&self, index: usize) -> Vector2<f32> {
        let offset = self.offset(index);
        let buf = self.buffer.buffer();
        let x = f32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap());
        let y = f32::from_le_bytes(buf[offset + 4..offset + 8].try_into().unwrap());
        [x, y]
    }
}

impl<'a> VertexBufferView<'a, Vector3<f32>> {
    pub fn get(&self, index: usize) -> Vector3<f32> {
        let offset = self.offset(index);
        let buf = self.buffer.buffer();
        let x = f32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap());
        let y = f32::from_le_bytes(buf[offset + 4..offset + 8].try_into().unwrap());
        let z = f32::from_le_bytes(buf[offset + 8..offset + 12].try_into().unwrap());
        [x, y, z]
    }
}

impl<'a> VertexBufferView<'a, Vector4<f32>> {
    pub fn get(&self, index: usize) -> Vector4<f32> {
        let offset = self.offset(index);
        let buf = self.buffer.buffer();
        let x = f32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap());
        let y = f32::from_le_bytes(buf[offset + 4..offset + 8].try_into().unwrap());
        let z = f32::from_le_bytes(buf[offset + 8..offset + 12].try_into().unwrap());
        let w = f32::from_le_bytes(buf[offset + 12..offset + 16].try_into().unwrap());
        [x, y, z, w]
    }
}

pub struct VertexBufferViewIter<'a, T> {
    view: &'a VertexBufferView<'a, T>,
    counter: usize,
}

macro_rules! impl_iter {
    ($t:ty) => {
        impl<'a> Iterator for VertexBufferViewIter<'a, $t> {
            type Item = $t;

            fn next(&mut self) -> Option<Self::Item> {
                if self.counter >= self.view.buffer.count() {
                    return None;
                }
                let item = self.view.get(self.counter);
                self.counter += 1;
                Some(item)
            }
        }
    };
}

impl_iter!(f32);
impl_iter!(Vector2<f32>);
impl_iter!(Vector3<f32>);
impl_iter!(Vector4<f32>);
