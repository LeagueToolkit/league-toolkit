use std::collections::BTreeMap;
use std::fmt::Debug;

use super::{
    ElementName, VertexBufferAccessor, VertexBufferDescription, VertexBufferUsage, VertexElement,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VertexBufferElementDescriptor {
    element: VertexElement,
    offset: isize,
}

impl VertexBufferElementDescriptor {
    pub fn new(element: VertexElement, offset: isize) -> Self {
        Self { element, offset }
    }

    pub fn element(&self) -> VertexElement {
        self.element
    }

    pub fn offset(&self) -> isize {
        self.offset
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VertexBuffer {
    description: VertexBufferDescription,
    elements: BTreeMap<ElementName, VertexBufferElementDescriptor>,

    stride: usize,
    count: usize,

    buffer: Vec<u8>,
}

impl VertexBuffer {
    pub fn new(usage: VertexBufferUsage, elements: Vec<VertexElement>, buffer: Vec<u8>) -> Self {
        if buffer.is_empty() {
            panic!("Buffer cannot be empty! FIXME (alan): don't panic here");
        }

        let description = VertexBufferDescription::new(usage, elements.clone());
        let mut element_descriptors = BTreeMap::new();
        let mut off = 0;
        for e in elements {
            if element_descriptors.contains_key(&e.name) {
                panic!("vertex buffer has duplicate elements! FIXME (alan): don't panic here :)");
            }
            element_descriptors.insert(e.name, VertexBufferElementDescriptor::new(e, off as isize));
            off += e.size()
        }
        let stride = off; // off collects the sizes of all the elements, which also happens to be the stride

        if buffer.len() % stride != 0 {
            panic!("Buffer size must be a multiple of it's stride! size: {}, stride: {stride} FIXME (alan): don't panic here", buffer.len());
        }
        Self {
            description,
            elements: element_descriptors,
            stride,
            count: buffer.len() / stride,
            buffer,
        }
    }

    pub fn accessor<T>(&self, element_name: ElementName) -> Option<VertexBufferAccessor<'_, T>> {
        self.elements
            .get(&element_name)
            .map(|desc| VertexBufferAccessor::<T>::new(desc.element, desc.offset as usize, self))
    }

    pub fn description(&self) -> &VertexBufferDescription {
        &self.description
    }

    pub fn elements(&self) -> &BTreeMap<ElementName, VertexBufferElementDescriptor> {
        &self.elements
    }

    pub fn stride(&self) -> usize {
        self.stride
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }
}
