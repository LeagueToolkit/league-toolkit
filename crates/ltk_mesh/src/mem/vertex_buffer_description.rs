use super::vertex::{ElementName, VertexElement};
use bitflags::bitflags;
use num_enum::{IntoPrimitive, TryFromPrimitive};

/// How often a vertex buffer's contents are expected to change.
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, TryFromPrimitive, IntoPrimitive)]
pub enum VertexBufferUsage {
    /// Written once, drawn many times.
    Static,
    /// Rewritten occasionally.
    Dynamic,
    /// Rewritten every frame.
    Stream,
}

bitflags! {
    /// The set of [`ElementName`]s a layout contains, as one bit each.
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
    pub struct VertexBufferElementFlags: u32 {
        /// Layout contains [`ElementName::Position`].
        const Position = 1 << (ElementName::Position as u32);
        /// Layout contains [`ElementName::BlendWeight`].
        const BlendWeight = 1 << (ElementName::BlendWeight as u32);
        /// Layout contains [`ElementName::Normal`].
        const Normal = 1 << (ElementName::Normal as u32);
        /// Layout contains [`ElementName::FogCoordinate`].
        const FogCoordinate = 1 << (ElementName::FogCoordinate as u32);
        /// Layout contains [`ElementName::PrimaryColor`].
        const PrimaryColor = 1 << (ElementName::PrimaryColor as u32);
        /// Layout contains [`ElementName::SecondaryColor`].
        const SecondaryColor = 1 << (ElementName::SecondaryColor as u32);
        /// Layout contains [`ElementName::BlendIndex`].
        const BlendIndex = 1 << (ElementName::BlendIndex as u32);
        /// Layout contains [`ElementName::Texcoord0`], the diffuse UV.
        const DiffuseUV = 1 << (ElementName::Texcoord0 as u32);
        /// Layout contains [`ElementName::Texcoord1`].
        const Texcoord1 = 1 << (ElementName::Texcoord1 as u32);
        /// Layout contains [`ElementName::Texcoord2`].
        const Texcoord2 = 1 << (ElementName::Texcoord2 as u32);
        /// Layout contains [`ElementName::Texcoord3`].
        const Texcoord3 = 1 << (ElementName::Texcoord3 as u32);
        /// Layout contains [`ElementName::Texcoord4`].
        const Texcoord4 = 1 << (ElementName::Texcoord4 as u32);
        /// Layout contains [`ElementName::Texcoord5`].
        const Texcoord5 = 1 << (ElementName::Texcoord5 as u32);
        /// Layout contains [`ElementName::Texcoord6`], which also carries tangents.
        const Texcoord6 = 1 << (ElementName::Texcoord6 as u32);
        /// Layout contains [`ElementName::Texcoord7`], the lightmap UV.
        const LightmapUV = 1 << (ElementName::Texcoord7 as u32);
    }
}

/// The elements, flags and usage of a [`VertexBuffer`](super::vertex::VertexBuffer).
///
/// Describes how one vertex is laid out; pair it with bytes via
/// [`VertexBuffer::new`](super::vertex::VertexBuffer::new).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VertexBufferDescription {
    usage: VertexBufferUsage,
    description_flags: VertexBufferElementFlags,
    elements: Vec<VertexElement>,
}

/// Folds element names into the bit set naming them.
///
/// # Panics
/// Panics if an [`ElementName`] has no corresponding flag, which cannot happen for the
/// variants the enum defines.
#[must_use]
pub fn get_element_flags(
    elements: impl IntoIterator<Item = ElementName>,
) -> VertexBufferElementFlags {
    let mut flags = VertexBufferElementFlags::empty();
    for e in elements {
        flags |= VertexBufferElementFlags::from_bits(1 << (e as u32))
            .unwrap_or_else(|| unreachable!("every ElementName has a flag, missing one for {e:?}"));
    }
    flags
}

impl VertexBufferDescription {
    /// Describes a vertex as an ordered list of elements.
    #[must_use]
    pub fn new(usage: VertexBufferUsage, elements: Vec<VertexElement>) -> Self {
        Self {
            usage,
            description_flags: get_element_flags(elements.iter().map(|e| e.name)),
            elements,
        }
    }

    /// The size in bytes of one vertex in this layout.
    #[must_use]
    pub fn vertex_size(&self) -> usize {
        self.elements.iter().map(|e| e.size()).sum()
    }

    /// How often the buffer is expected to change.
    #[must_use]
    pub fn usage(&self) -> VertexBufferUsage {
        self.usage
    }

    /// The set of elements this layout contains.
    #[must_use]
    pub fn description_flags(&self) -> VertexBufferElementFlags {
        self.description_flags
    }

    /// The elements in layout order.
    #[must_use]
    pub fn elements(&self) -> &[VertexElement] {
        &self.elements
    }
}
