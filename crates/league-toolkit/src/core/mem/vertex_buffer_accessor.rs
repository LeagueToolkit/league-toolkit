use std::marker::PhantomData;
use glam::{Vec2, vec2, Vec3, vec3, Vec4, vec4};

use crate::core::mem::ElementFormat;

use super::{VertexBuffer, VertexElement};

macro_rules! repack_self {
    ($self:ident) => {
        VertexBufferAccessor {
            buffer: $self.buffer,
            element: $self.element,
            element_off: $self.element_off,
            _t: PhantomData,
        }
    };
}

/// A view of a single VertexElement over a VertexBuffer
pub struct VertexBufferAccessor<'a, T> {
    buffer: &'a VertexBuffer,
    element: VertexElement,
    element_off: usize,

    _t: PhantomData<T>,
}
impl<'a, T> VertexBufferAccessor<'a, T> {
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
    fn offset(&self, index: usize) -> usize {
        self.buffer.stride() * index + self.element_off
    }

    pub fn iter(&'a self) -> VertexBufferViewIter<'a, T> {
        VertexBufferViewIter {
            view: self,
            counter: 0,
        }
    }
    // TODO (alan): impl the rest of the ElementFormat's
}

// TODO(alan): figure out endianness (again)

impl<'a> VertexBufferAccessor<'a, f32> {
    pub fn get(&self, index: usize) -> f32 {
        let offset = self.offset(index);
        let buf = self.buffer.buffer();
        f32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap())
    }
}

impl<'a> VertexBufferAccessor<'a, Vec2> {
    pub fn get(&self, index: usize) -> Vec2 {
        let offset = self.offset(index);
        let buf = self.buffer.buffer();
        let x = f32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap());
        let y = f32::from_le_bytes(buf[offset + 4..offset + 8].try_into().unwrap());
        vec2(x, y)
    }
}

impl<'a> VertexBufferAccessor<'a, Vec3> {
    pub fn get(&self, index: usize) -> Vec3 {
        let offset = self.offset(index);
        let buf = self.buffer.buffer();
        let x = f32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap());
        let y = f32::from_le_bytes(buf[offset + 4..offset + 8].try_into().unwrap());
        let z = f32::from_le_bytes(buf[offset + 8..offset + 12].try_into().unwrap());
        vec3(x, y, z)
    }
}

impl<'a> VertexBufferAccessor<'a, Vec4> {
    pub fn get(&self, index: usize) -> Vec4 {
        let offset = self.offset(index);
        let buf = self.buffer.buffer();
        let x = f32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap());
        let y = f32::from_le_bytes(buf[offset + 4..offset + 8].try_into().unwrap());
        let z = f32::from_le_bytes(buf[offset + 8..offset + 12].try_into().unwrap());
        let w = f32::from_le_bytes(buf[offset + 12..offset + 16].try_into().unwrap());
        vec4(x, y, z, w)
    }
}

pub struct VertexBufferViewIter<'a, T> {
    view: &'a VertexBufferAccessor<'a, T>,
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
impl_iter!(Vec2);
impl_iter!(Vec3);
impl_iter!(Vec4);
