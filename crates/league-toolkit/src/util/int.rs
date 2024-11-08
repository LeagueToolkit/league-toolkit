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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u24_new() {
        let value = u24::new(0x123456);
        assert_eq!(value.hi, 0x12);
        assert_eq!(value.mi, 0x34);
        assert_eq!(value.lo, 0x56);
    }

    #[test]
    fn test_u24_from_u32() {
        let value: u24 = 0x123456_u32.into();
        assert_eq!(value.hi, 0x12);
        assert_eq!(value.mi, 0x34);
        assert_eq!(value.lo, 0x56);
    }

    #[test]
    fn test_u32_from_u24() {
        let u24_value = u24::new(0x123456);
        let u32_value: u32 = u24_value.into();
        assert_eq!(u32_value, 0x123456);
    }
}
