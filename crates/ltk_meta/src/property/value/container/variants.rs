macro_rules! variants {
    ($macro:ident $(, $args:tt)*) => {
        $macro! {
            $( $args )*
            None(crate::property::value::None),
            Bool(crate::property::value::Bool),
            I8(crate::property::value::I8),
            U8(crate::property::value::U8),
            I16(crate::property::value::I16),
            U16(crate::property::value::U16),
            I32(crate::property::value::I32),
            U32(crate::property::value::U32),
            I64(crate::property::value::I64),
            U64(crate::property::value::U64),
            F32(crate::property::value::F32),
            Vector2(crate::property::value::Vector2),
            Vector3(crate::property::value::Vector3),
            Vector4(crate::property::value::Vector4),
            Matrix44(crate::property::value::Matrix44),
            Color(crate::property::value::Color),
            String(crate::property::value::String),
            Hash(crate::property::value::Hash),
            WadChunkLink(crate::property::value::WadChunkLink),
            Struct(crate::property::value::Struct),
            Embedded(crate::property::value::Embedded),
            ObjectLink(crate::property::value::ObjectLink),
            BitBool(crate::property::value::BitBool),
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
                crate::property::value::Container::$variant($inner) => $body,
            )*
        }
    };
}

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
