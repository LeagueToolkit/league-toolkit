use std::{fmt::Debug, mem::size_of};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IndexFormat {
    U16,
    U32,
}

impl IndexFormat {
    pub fn size(&self) -> usize {
        match self {
            IndexFormat::U16 => size_of::<u16>(),
            IndexFormat::U32 => size_of::<u32>(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IndexBuffer {
    format: IndexFormat,
    count: usize,
    stride: usize,

    buffer: Vec<u8>,
}

impl Debug for IndexBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndexBuffer")
            .field("format", &self.format)
            .field("count", &self.count)
            .field("stride", &self.stride)
            .field("buffer (size)", &self.buffer.len())
            .finish()
    }
}

impl IndexBuffer {
    pub fn new(format: IndexFormat, buffer: Vec<u8>) -> Self {
        let stride = format.size();
        if buffer.len() % stride != 0 {
            panic!("Index buffer size must be a multiple of index size!");
        }
        Self {
            format,
            count: buffer.len() / stride,
            stride,
            buffer,
        }
    }

    pub fn get(&self, index: usize) -> u32 {
        let off = index * self.stride;
        match self.format {
            IndexFormat::U16 => {
                u16::from_le_bytes(self.buffer[off..off + 2].try_into().unwrap()).into()
            }
            IndexFormat::U32 => u32::from_le_bytes(self.buffer[off..off + 4].try_into().unwrap()),
        }
    }

    pub fn iter(&self) -> IndexBufferIter {
        IndexBufferIter {
            buffer: self,
            counter: 0,
        }
    }

    pub fn format(&self) -> &IndexFormat {
        &self.format
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn stride(&self) -> usize {
        self.stride
    }

    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }
}

pub struct IndexBufferIter<'a> {
    buffer: &'a IndexBuffer,
    counter: usize,
}

impl<'a> Iterator for IndexBufferIter<'a> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.counter >= self.buffer.count {
            return None;
        }
        let item = self.buffer.get(self.counter);
        self.counter += 1;
        Some(item)
    }
}
