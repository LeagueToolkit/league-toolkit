//! Error types for navigation grid reading and writing.

/// Errors that can occur when reading a navigation grid or an overlay.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ParseError {
    /// The grid version is not 6 or 7.
    #[error("unsupported navigation grid version: {0}")]
    UnsupportedVersion(u8),

    /// A width multiplied by a height does not fit in `usize`.
    #[error("{width} x {height} elements do not fit in memory")]
    CountOverflow { width: u32, height: u32 },

    /// An IO error occurred.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Errors that can occur when writing an overlay.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WriteError {
    /// An overlay has more rectangles than its 8-bit count holds.
    #[error("the overlay has {0} rectangles, more than 255")]
    TooManyRects(usize),

    /// The value count of a rectangle is not its width multiplied by its height.
    #[error("rectangle {rect} holds {actual} values, but its size needs {expected}")]
    RectSizeMismatch {
        rect: usize,
        expected: u64,
        actual: usize,
    },

    /// An IO error occurred.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
