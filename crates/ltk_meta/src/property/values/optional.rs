use crate::{
    property::{values, Kind, NoMeta},
    traits::{PropertyExt, PropertyValueExt, ReadProperty, ReaderExt, WriteProperty, WriterExt},
    Error, PropertyValueEnum,
};
use ltk_io_ext::{ReaderExt as _, WriterExt as _};

macro_rules! construct_enum {
    ([$( $variant:ident, )*]) => {
        #[cfg_attr(
            feature = "serde",
            derive(serde::Serialize, serde::Deserialize),
            serde(bound = "for <'dee> M: serde::Serialize + serde::Deserialize<'dee>")
        )]
        #[derive(Clone, PartialEq, Debug)]
        pub enum Optional<M = NoMeta> {
            $($variant{
                value: Option<values::$variant<M>>,
                meta: M
            },)*
        }

        impl<M> Optional<M> {
            #[inline(always)]
            #[must_use]
            /// Helper function to create an empty [`Optional`], if the property kind can be stored in one.
            pub fn empty(kind: Kind) -> Option<Self> where M: Default {
                Some(match kind {
                    $(Kind::$variant => Self::$variant{value: None, meta: M::default()},)*
                    _ => return None
                })
            }

            #[inline(always)]
            #[must_use]
            pub fn no_meta(self) -> Optional<NoMeta> {
                match self {
                    $(Self::$variant{ value, meta: _ } => Optional::$variant{ value: value.map(|v| v.no_meta()), meta: NoMeta },)*
                }
            }
        }

        impl<M> PropertyExt for Optional<M> {
            fn size_no_header(&self) -> usize {
                2 + match &self {
                    $(Self::$variant{value, ..} => value.as_ref().map(|i| i.size_no_header()).unwrap_or_default(),)*
                }
            }
            type Meta = M;
            fn meta(&self) -> &Self::Meta {
                match &self {
                    $(Self::$variant{meta, ..} => meta,)*
                }
            }
            fn meta_mut(&mut self) -> &mut Self::Meta {
                match self {
                    $(Self::$variant{meta, ..} => {
                        meta
                    })*
                }
            }
        }

        $(
            impl<M: Default> From<Option<values::$variant<M>>> for Optional<M> {
                fn from(other: Option<values::$variant<M>>) -> Self {
                    Self::$variant{value: other, meta: M::default()}
                }

            }
        )*
        $(
            impl<M: Default> From<values::$variant<M>> for Optional<M> {
                fn from(other: values::$variant<M>) -> Self {
                    Self::$variant{value: Some(other), meta: M::default()}
                }

            }
        )*

        impl<M> Optional<M> {
            #[inline(always)]
            pub fn new(item_kind: Kind, value: Option<PropertyValueEnum<M>>) -> Result<Self, Error> where M: Default {
                Self::new_with_meta(item_kind, value, M::default())
            }
            #[inline(always)]
            pub fn new_with_meta(item_kind: Kind, value: Option<PropertyValueEnum<M>>, meta: M) -> Result<Self, Error> {
                match item_kind {
                    $(Kind::$variant => match value {
                        Some(PropertyValueEnum::$variant(inner)) => Ok(Self::$variant{value: Some(inner), meta}),
                        None => Ok(Self::$variant{value: None, meta}),
                        Some(value) => Err(Error::MismatchedContainerTypes {expected: item_kind, got: value.kind()}),
                    },)*
                    kind => Err(Error::InvalidNesting(kind)),
                }
            }

            #[inline(always)]
            #[must_use]
            pub fn into_parts(self) -> (Option<PropertyValueEnum<M>>, M) {
                match self {
                    $(Optional::$variant{ value, meta } => (value.map(PropertyValueEnum::$variant), meta),)*
                }
            }

            #[inline(always)]
            #[must_use]
            pub fn is_some(&self) -> bool {
                match self {
                    $(Self::$variant{value,..} => value.is_some(),)*
                }
            }

            #[inline(always)]
            #[must_use]
            pub fn is_none(&self) -> bool {
                match self {
                    $(Self::$variant{value,..} => value.is_none(),)*
                }
            }
        }
    };
}

container_variants!(construct_enum);

impl<M: Default> Default for Optional<M> {
    fn default() -> Self {
        Self::None {
            value: None,
            meta: M::default(),
        }
    }
}

impl<M> Optional<M> {
    pub fn empty_const<T>() -> Self
    where
        Self: From<Option<T>>,
    {
        Self::from(None)
    }

    #[inline(always)]
    #[must_use]
    pub fn item_kind(&self) -> Kind {
        container_variants!(property_kinds, (self))
    }

    pub fn into_inner(self) -> Option<PropertyValueEnum<M>> {
        self.into_parts().0
    }
    pub fn into_meta(self) -> M {
        self.into_parts().1
    }
}

impl<M> PropertyValueExt for Optional<M> {
    const KIND: Kind = Kind::Optional;
}

impl<M: Default> ReadProperty for Optional<M> {
    fn from_reader<R: std::io::Read + std::io::Seek + ?Sized>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, Error> {
        let kind = reader.read_property_kind(legacy)?;
        if kind.is_container() {
            return Err(Error::InvalidNesting(kind));
        }

        let is_some = reader.read_bool()?;

        macro_rules! read_inner {
            (($value:expr)
             [$( $variant:ident, )*]) => {
                match $value {
                    $(
                        Kind::$variant => match is_some {
                            true => Ok(values::$variant::from_reader(reader, legacy)?.into()),
                            false => Ok(Self::empty_const::<values::$variant<M>>()),
                        },
                    )*
                    kind => { return Err(Error::InvalidNesting(kind)) }
                }
            };
        }

        container_variants!(read_inner, (kind))
    }
}
impl<M: Clone> WriteProperty for Optional<M> {
    fn to_writer<R: std::io::Write + std::io::Seek + ?Sized>(
        &self,
        writer: &mut R,
        legacy: bool,
    ) -> Result<(), std::io::Error> {
        if legacy {
            unimplemented!("legacy optional write")
        }
        writer.write_property_kind(self.item_kind())?;
        writer.write_bool(self.is_some())?;

        macro_rules! write_inner {
            (($value:expr)
             [$( $variant:ident, )*]) => {
                match $value {
                    $(
                        Self::$variant{value: Some(value),..} => value.to_writer(writer, legacy),

                    )*
                    _ => { Ok(()) }
                }
            };
        }

        container_variants!(write_inner, (self))
    }
}
