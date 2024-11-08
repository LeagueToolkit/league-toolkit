#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct u24 {
    lo: u8,
    mi: u8,
    hi: u8,
}

impl u24 {
    pub fn new(value: u32) -> Self {
        Self {
            lo: (value & 0xFF) as u8,
            mi: ((value >> 8) & 0xFF) as u8,
            hi: ((value >> 16) & 0xFF) as u8,
        }
    }
}

impl From<u32> for u24 {
    fn from(value: u32) -> Self {
        Self::new(value)
    }
}

impl From<u24> for u32 {
    fn from(value: u24) -> Self {
        (value.hi as u32) << 16 | (value.mi as u32) << 8 | (value.lo as u32)
    }
}
