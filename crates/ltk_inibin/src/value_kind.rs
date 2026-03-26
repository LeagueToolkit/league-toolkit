bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ValueKind: u16 {
        const INT32_LIST    = 1 << 0;
        const F32_LIST      = 1 << 1;
        const U8_LIST       = 1 << 2;
        const INT16_LIST    = 1 << 3;
        const INT8_LIST     = 1 << 4;
        const BIT_LIST      = 1 << 5;
        const VEC3_U8_LIST  = 1 << 6;
        const VEC3_F32_LIST = 1 << 7;
        const VEC2_U8_LIST  = 1 << 8;
        const VEC2_F32_LIST = 1 << 9;
        const VEC4_U8_LIST  = 1 << 10;
        const VEC4_F32_LIST = 1 << 11;
        const STRING_LIST   = 1 << 12;
        const INT64_LIST    = 1 << 13;
    }
}

/// Non-string kinds in bit order. STRING_LIST is always read/written last.
pub(crate) const NON_STRING_KINDS: [ValueKind; 13] = [
    ValueKind::INT32_LIST,
    ValueKind::F32_LIST,
    ValueKind::U8_LIST,
    ValueKind::INT16_LIST,
    ValueKind::INT8_LIST,
    ValueKind::BIT_LIST,
    ValueKind::VEC3_U8_LIST,
    ValueKind::VEC3_F32_LIST,
    ValueKind::VEC2_U8_LIST,
    ValueKind::VEC2_F32_LIST,
    ValueKind::VEC4_U8_LIST,
    ValueKind::VEC4_F32_LIST,
    ValueKind::INT64_LIST,
];
