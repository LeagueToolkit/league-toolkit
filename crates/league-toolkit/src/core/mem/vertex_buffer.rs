use std::collections::{HashMap, HashSet};

use super::{
    ElementName, VertexBufferDescription, VertexBufferUsage, VertexBufferView, VertexElement,
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

#[derive(Debug, Clone)]
pub struct VertexBuffer {
    description: VertexBufferDescription,
    elements: HashMap<ElementName, VertexBufferElementDescriptor>,

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
        let mut element_descriptors = HashMap::new();
        let mut off = 0;
        for e in sanitize_elements_for_duplication(elements) {
            element_descriptors.insert(e.name, VertexBufferElementDescriptor::new(e, off as isize));
            off += e.size()
        }
        let stride = off; // off collects the sizes of all the elements, which also happens to be the stride

        if buffer.len() % stride != 0 {
            panic!("Buffer size must be a multiple of it's stride! FIXME (alan): don't panic here");
        }
        Self {
            description,
            elements: element_descriptors,
            stride,
            count: buffer.len() / stride,
            buffer,
        }
    }

    pub fn view(&self, element_name: ElementName) -> Option<VertexBufferView<'_, ()>> {
        self.elements
            .get(&element_name)
            .map(|desc| VertexBufferView::<()>::new(desc.element, desc.offset as usize, self))
    }

    pub fn description(&self) -> &VertexBufferDescription {
        &self.description
    }

    pub fn elements(&self) -> &HashMap<ElementName, VertexBufferElementDescriptor> {
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

fn sanitize_elements_for_duplication(elements: Vec<VertexElement>) -> HashSet<VertexElement> {
    let orig_len = elements.len();
    let mut set: HashSet<VertexElement> = HashSet::new();
    elements.into_iter().for_each(|x| {
        set.insert(x);
    });
    if set.len() != orig_len {
        panic!("Contains duplicate vertex elements. FIXME (alan): don't panic here :)");
    }
    set
}
