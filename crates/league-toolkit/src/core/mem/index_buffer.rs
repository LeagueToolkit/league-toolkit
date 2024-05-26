use std::mem::size_of;

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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IndexBuffer {
    format: IndexFormat,
    count: usize,
    stride: usize,

    buffer: Vec<u8>,
}

impl IndexBuffer {
    pub fn new(format: IndexFormat, buffer: Vec<u8>) -> Self {
        let stride = format.size();
        Self {
            format,
            count: buffer.len() / stride,
            stride,
            buffer,
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
