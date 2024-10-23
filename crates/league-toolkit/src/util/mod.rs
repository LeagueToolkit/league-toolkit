pub mod hash;
pub mod reader;
pub mod writer;

pub use reader::*;
pub use writer::*;

/// Returns the number of bytes 'moved' by the `inner` function, of some seekable object.
/// **NOTE**: this function assumes the cursor position after calling `inner()` will always be >= the cursor after calling `inner()`
///
/// The `inner` function's Err type **must** impl `From<std::io::Error>`, since
/// `std::io::Seek::stream_position()` is fallable
pub fn measure<S, T, E>(
    seekable: &mut S,
    mut inner: impl FnMut(&mut S) -> Result<T, E>,
) -> Result<(u64, T), E>
where
    S: std::io::Seek + ?Sized,
    E: std::error::Error + From<std::io::Error>,
{
    let start = seekable.stream_position()?;

    let val = inner(seekable)?;
    Ok((seekable.stream_position()?.saturating_sub(start), val))
}
