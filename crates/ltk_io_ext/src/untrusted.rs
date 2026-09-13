//! Buffers sized from counts read from a stream.
//!
//! A count read from a file is untrusted: a few bytes can declare billions of elements. A
//! collection sized from such a count reserves at most [`MAX_RESERVE`] bytes up front and
//! grows as its elements are read. A stream that ends early fails the read with
//! [`io::ErrorKind::UnexpectedEof`], and the collection never outgrows the bytes read by
//! more than its growth factor.
//!
//! # Examples
//!
//! ```
//! use byteorder::{ReadBytesExt, LE};
//! use ltk_io_ext::untrusted::UntrustedCapacity;
//! use std::io::{self, Cursor};
//!
//! // A count of u32::MAX, followed by only two elements.
//! let mut reader = Cursor::new([0xFF, 0xFF, 0xFF, 0xFF, 1, 0, 2, 0]);
//! let count = reader.read_u32::<LE>()? as usize;
//!
//! let mut elements = Vec::with_untrusted_capacity(count);
//! let result: io::Result<()> = (0..count).try_for_each(|_| {
//!     elements.push(reader.read_u16::<LE>()?);
//!     Ok(())
//! });
//!
//! assert_eq!(elements, [1, 2]);
//! assert_eq!(result.unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
//! # Ok::<(), io::Error>(())
//! ```

use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{BuildHasher, Hash};
use std::io::{self, Read};
use std::mem::size_of;

/// The most bytes a collection reserves before any of its elements are read: 1 MiB.
pub const MAX_RESERVE: usize = 1 << 20;

/// Returns `count`, capped at [`MAX_RESERVE`] bytes of elements of `element_size` bytes.
///
/// A zero-sized element counts as one byte.
fn capped(count: usize, element_size: usize) -> usize {
    count.min(MAX_RESERVE / element_size.max(1))
}

/// A collection that reserves capacity for a count read from a stream.
pub trait UntrustedCapacity {
    /// Creates an empty collection with capacity for `count` elements.
    ///
    /// The capacity is capped at [`MAX_RESERVE`] bytes of elements. The collection grows
    /// past it as elements are added.
    #[must_use]
    fn with_untrusted_capacity(count: usize) -> Self;
}

impl<T> UntrustedCapacity for Vec<T> {
    fn with_untrusted_capacity(count: usize) -> Self {
        Vec::with_capacity(capped(count, size_of::<T>()))
    }
}

impl<T> UntrustedCapacity for VecDeque<T> {
    fn with_untrusted_capacity(count: usize) -> Self {
        VecDeque::with_capacity(capped(count, size_of::<T>()))
    }
}

impl UntrustedCapacity for String {
    fn with_untrusted_capacity(count: usize) -> Self {
        String::with_capacity(capped(count, 1))
    }
}

impl<K: Eq + Hash, V, S: BuildHasher + Default> UntrustedCapacity for HashMap<K, V, S> {
    fn with_untrusted_capacity(count: usize) -> Self {
        HashMap::with_capacity_and_hasher(capped(count, size_of::<(K, V)>()), S::default())
    }
}

impl<T: Eq + Hash, S: BuildHasher + Default> UntrustedCapacity for HashSet<T, S> {
    fn with_untrusted_capacity(count: usize) -> Self {
        HashSet::with_capacity_and_hasher(capped(count, size_of::<T>()), S::default())
    }
}

/// Reads exactly `len` bytes into a new buffer.
///
/// The buffer grows with the bytes read, not with `len`.
///
/// # Errors
///
/// Returns [`io::ErrorKind::UnexpectedEof`] when the stream ends before `len` bytes, and any
/// error the reader returns.
pub fn read_bytes<R: Read + ?Sized>(reader: &mut R, len: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::with_untrusted_capacity(len);
    Read::take(&mut *reader, len as u64).read_to_end(&mut bytes)?;
    if bytes.len() == len {
        Ok(bytes)
    } else {
        Err(io::ErrorKind::UnexpectedEof.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn vec_capacity_is_the_count_below_the_cap() {
        assert_eq!(Vec::<u32>::with_untrusted_capacity(100).capacity(), 100);
    }

    #[test]
    fn vec_capacity_is_capped_at_max_reserve_bytes() {
        let vec = Vec::<u32>::with_untrusted_capacity(usize::MAX);

        assert_eq!(vec.capacity(), MAX_RESERVE / 4);
    }

    #[test]
    fn string_capacity_is_capped_at_max_reserve_bytes() {
        assert_eq!(
            String::with_untrusted_capacity(usize::MAX).capacity(),
            MAX_RESERVE
        );
    }

    #[test]
    fn hash_map_capacity_is_capped_at_max_reserve_elements() {
        let map = HashMap::<u32, u32>::with_untrusted_capacity(usize::MAX);

        assert!(map.capacity() >= MAX_RESERVE / 8);
        assert!(map.capacity() < MAX_RESERVE / 2);
    }

    #[test]
    fn capped_counts_a_zero_sized_element_as_one_byte() {
        assert_eq!(capped(usize::MAX, 0), MAX_RESERVE);
    }

    #[test]
    fn read_bytes_reads_exactly_len_bytes() {
        let mut reader = Cursor::new([1, 2, 3, 4]);

        assert_eq!(read_bytes(&mut reader, 3).unwrap(), [1, 2, 3]);
        assert_eq!(reader.position(), 3);
    }

    #[test]
    fn read_bytes_rejects_a_stream_shorter_than_len() {
        let error = read_bytes(&mut Cursor::new([1, 2, 3, 4]), usize::MAX).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    }
}
