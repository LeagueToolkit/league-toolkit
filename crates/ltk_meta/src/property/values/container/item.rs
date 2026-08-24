use crate::{property::values, traits::PropertyValueExt};

/// A value type that can sit inside a [`Container`], an [`UnorderedContainer`] or an [`Optional`].
///
/// The format has no nested containers, so every value type implements this except the four that
/// are containers themselves: [`Container`], [`UnorderedContainer`], [`Optional`] and [`Map`].
/// Equivalently, [`Kind::is_container`] is false for every implementor's [`PropertyValueExt::KIND`].
///
/// It is a marker, so it adds nothing to [`PropertyValueExt`]. Its job is to let
/// [`Container::from_iter`] and the [`Optional`] conversions reject a nested container at compile
/// time; the constructors that take a [`Kind`] check the same rule at run time.
///
/// [`Container`]: super::Container
/// [`Container::from_iter`]: super::Container::from_iter
/// [`UnorderedContainer`]: values::UnorderedContainer
/// [`Optional`]: values::Optional
/// [`Map`]: values::Map
/// [`Kind`]: crate::property::Kind
/// [`Kind::is_container`]: crate::property::Kind::is_container
pub trait ContainerItem: PropertyValueExt {}

macro_rules! impl_container_item {
    ($($variant:ident,)*) => {
        $(impl<M> ContainerItem for values::$variant<M> {})*
    };
}

impl_container_item! {
    None,
    Bool, BitBool,
    I8, U8,
    I16, U16,
    I32, U32,
    I64, U64,
    F32,
    Vector2, Vector3, Vector4,
    Matrix44,
    Color,
    String,
    Hash,
    WadChunkLink,
    Struct,
    Embedded,
    ObjectLink,
}
