macro_rules! variants {
    ($macro:ident $(, $args:tt)*) => {
        $macro! {
            $( $args )*
            None(crate::property::values::None),
            Bool(crate::property::values::Bool),
            I8(crate::property::values::I8),
            U8(crate::property::values::U8),
            I16(crate::property::values::I16),
            U16(crate::property::values::U16),
            I32(crate::property::values::I32),
            U32(crate::property::values::U32),
            I64(crate::property::values::I64),
            U64(crate::property::values::U64),
            F32(crate::property::values::F32),
            Vector2(crate::property::values::Vector2),
            Vector3(crate::property::values::Vector3),
            Vector4(crate::property::values::Vector4),
            Matrix44(crate::property::values::Matrix44),
            Color(crate::property::values::Color),
            String(crate::property::values::String),
            Hash(crate::property::values::Hash),
            WadChunkLink(crate::property::values::WadChunkLink),
            Struct(crate::property::values::Struct),
            Embedded(crate::property::values::Embedded),
            ObjectLink(crate::property::values::ObjectLink),
            BitBool(crate::property::values::BitBool),
        }
    };
}

macro_rules! match_property {
    ($value:expr, |$inner:ident| $body:expr) => {
        variants!(match_property_arms, ($value, |$inner| $body))
    };
}

macro_rules! match_property_arms {
    (($value:expr, |$inner:ident| $body:expr)
     $( $variant:ident($ty:ty), )*) => {
        match $value {
            $(
                crate::property::values::Container::$variant($inner) => $body,
            )*
        }
    };
}

// macro_rules! match_enum_inner {
//     (($value:expr, $on:ident, ||, $body:expr)
//      [$( $variant:ident, )*] ) => {
//         match $value {
//             $(
//                 $on::$variant => $body,
//             )*
//         }
//     };
//     (($value:expr, $on:ident, |$inner:ident| $body:expr)
//      [$( $variant:ident, )*] ) => {
//         match $value {
//             $(
//                 $on::$variant($inner) => $body,
//             )*
//         }
//     };
// }
// macro_rules! match_enum {
//     ($value:expr, $on:ident, ||, $body:expr) => {
//         variants!(match_enum_inner, ($value, $on, $body))
//     };
//     ($value:expr, $on:ident, |$inner:ident| $body:expr) => {
//         variants!(match_enum_inner, ($value, $on, |$inner| $body))
//     };
// }

macro_rules! property_kinds {
    (($value:expr)
     $( $variant:ident($ty:ty), )* ) => {
        match $value {
            $(
                Container::$variant(_) => <$ty>::KIND,
            )*
        }
    };
}
