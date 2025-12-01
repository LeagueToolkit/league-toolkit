use byteorder::{ReadBytesExt, LE};
use std::io;
use std::io::Read;

// Represents the optimization settings of a transform component
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ErrorMetric {
    /// The max allowed error
    pub margin: f32,
    /// The distance at which the error is measured
    pub discontinuity_threshold: f32,
}

impl Default for ErrorMetric {
    fn default() -> Self {
        Self {
            margin: 2.0,
            discontinuity_threshold: 10.0,
        }
    }
}

impl ErrorMetric {
    pub fn new(margin: f32, discontinuity_threshold: f32) -> Self {
        Self {
            margin,
            discontinuity_threshold,
        }
    }

    pub fn from_reader<R: Read + ?Sized>(reader: &mut R) -> io::Result<Self> {
        Ok(Self::new(
            reader.read_f32::<LE>()?,
            reader.read_f32::<LE>()?,
        ))
    }
}
