//! Index buffer types.
//!
//! An [`IndexBuffer`] wraps the raw bytes of a mesh's index data, in either of the two widths
//! the League formats use. The width is a type parameter rather than a runtime tag, so a
//! `u16` buffer and a `u32` buffer are distinct types and cannot be mixed up.
use std::{fmt::Debug, io, marker::PhantomData, mem::size_of};

/// Reads and writes one index of a given width.
///
/// Implemented for [`u16`] and [`u32`]; there is no reason to implement it elsewhere.
pub trait Format {
    /// The value one index decodes to.
    type Item;

    /// Reads the index at `index` from a packed buffer.
    ///
    /// # Panics
    /// Panics if `index` is out of bounds for `buf`.
    #[must_use]
    fn get(buf: &[u8], index: usize) -> Self::Item;

    /// Writes the index at `index` into a packed buffer.
    ///
    /// # Panics
    /// Panics if `index` is out of bounds for `buf`.
    fn set(buf: &mut [u8], index: usize, value: Self::Item);
}

/// A buffer of mesh indices, either `u16` or `u32` wide.
///
/// # Examples
/// ```
/// use ltk_mesh::mem::IndexBuffer;
///
/// let indices = IndexBuffer::<u16>::new(vec![0, 0, 1, 0, 2, 0]);
/// assert_eq!(indices.count(), 3);
/// assert_eq!(indices.iter().collect::<Vec<_>>(), [0, 1, 2]);
/// ```
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IndexBuffer<F: Format> {
    count: usize,

    buffer: Vec<u8>,

    _format: PhantomData<F>,
}

impl<F: Format> Debug for IndexBuffer<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndexBuffer")
            .field("count", &self.count)
            .field("stride", &self.stride())
            .field("buffer (size)", &self.buffer.len())
            .finish()
    }
}

impl Format for u32 {
    type Item = u32;
    fn get(buf: &[u8], index: usize) -> u32 {
        let off = index * size_of::<u32>();
        u32::from_le_bytes(buf[off..off + 4].try_into().unwrap())
    }
    fn set(buf: &mut [u8], index: usize, value: u32) {
        let off = index * size_of::<u32>();
        buf[off..off + 4].copy_from_slice(&value.to_le_bytes());
    }
}
impl Format for u16 {
    type Item = u16;
    fn get(buf: &[u8], index: usize) -> u16 {
        let off = index * size_of::<u16>();
        u16::from_le_bytes(buf[off..off + 2].try_into().unwrap())
    }
    fn set(buf: &mut [u8], index: usize, value: u16) {
        let off = index * size_of::<u16>();
        buf[off..off + 2].copy_from_slice(&value.to_le_bytes());
    }
}
impl<F: Format> IndexBuffer<F> {
    /// Creates an index buffer from packed little endian bytes.
    ///
    /// # Panics
    /// Panics if `buffer` is not a whole number of indices long.
    #[must_use]
    pub fn new(buffer: Vec<u8>) -> Self {
        let stride = size_of::<F>();
        assert!(
            buffer.len().is_multiple_of(stride),
            "index buffer size must be a multiple of the index size: got {} bytes, stride is {stride}",
            buffer.len()
        );
        Self {
            count: buffer.len() / stride,
            buffer,
            _format: PhantomData,
        }
    }

    /// Reads `count` indices from a reader.
    ///
    /// # Errors
    /// Returns the reader's error, including [`io::ErrorKind::UnexpectedEof`] if fewer than
    /// `count` indices are available.
    ///
    /// # Arguments
    /// * `reader` - The reader to read from.
    /// * `count` - The number of indices to read.
    pub fn read<R: io::Read>(reader: &mut R, count: usize) -> Result<Self, io::Error> {
        let mut buffer = vec![0u8; size_of::<F>() * count];
        reader.read_exact(&mut buffer)?;
        Ok(Self::new(buffer))
    }

    /// The size in bytes of a single index.
    #[inline(always)]
    #[must_use]
    pub fn stride(&self) -> usize {
        size_of::<F>()
    }

    /// An iterator over the indices in the buffer.
    #[inline(always)]
    #[must_use]
    pub fn iter(&self) -> IndexBufferIter<'_, F> {
        IndexBufferIter {
            buffer: self,
            counter: 0,
        }
    }

    /// Reads the index at `index`.
    ///
    /// # Panics
    /// Panics if `index` is out of bounds.
    #[inline]
    #[must_use]
    pub fn get(&self, index: usize) -> F::Item {
        F::get(&self.buffer, index)
    }

    /// Overwrites the index at `index`.
    ///
    /// # Panics
    /// Panics if `index` is out of bounds.
    #[inline]
    pub fn set(&mut self, index: usize, value: F::Item) {
        F::set(&mut self.buffer, index, value);
    }

    /// The number of indices in the buffer.
    #[inline(always)]
    #[must_use]
    pub fn count(&self) -> usize {
        self.count
    }

    /// Whether the buffer holds no indices.
    #[inline(always)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The raw underlying bytes, ready to upload without a copy.
    #[inline(always)]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.buffer
    }

    /// Takes ownership of the underlying bytes.
    #[inline(always)]
    #[must_use]
    pub fn into_inner(self) -> Vec<u8> {
        self.buffer
    }
}

impl<'a, F: Format> IntoIterator for &'a IndexBuffer<F> {
    type Item = F::Item;
    type IntoIter = IndexBufferIter<'a, F>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator over the indices of an [`IndexBuffer`], created by [`IndexBuffer::iter`].
pub struct IndexBufferIter<'a, F: Format> {
    buffer: &'a IndexBuffer<F>,
    counter: usize,
}

impl<F: Format> Debug for IndexBufferIter<'_, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndexBufferIter")
            .field("position", &self.counter)
            .field("count", &self.buffer.count())
            .finish()
    }
}

impl<F: Format> Iterator for IndexBufferIter<'_, F> {
    type Item = F::Item;

    fn next(&mut self) -> Option<Self::Item> {
        if self.counter >= self.buffer.count {
            return None;
        }
        let item = F::get(self.buffer.as_bytes(), self.counter);
        self.counter += 1;
        Some(item)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.buffer.count.saturating_sub(self.counter);
        (remaining, Some(remaining))
    }
}

impl<F: Format> ExactSizeIterator for IndexBufferIter<'_, F> {}
