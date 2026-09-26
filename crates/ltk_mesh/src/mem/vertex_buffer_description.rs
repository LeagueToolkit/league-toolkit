use std::io::{self, Read, Write};

use super::vertex::{ElementFormat, ElementName, VertexElement};
use crate::error::ParseError;
use bitflags::bitflags;
use byteorder::{ReadBytesExt, WriteBytesExt, LE};
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
    /// The number of element slots in a serialized description.
    ///
    /// A serialized description is 128 bytes: a `u32` usage, a `u32` element count, and this
    /// many slots of 8 bytes each.
    pub const SERIALIZED_ELEMENT_SLOTS: usize = 15;

    /// The element the game writes into a slot past the element count.
    ///
    /// It is a default-constructed `Riot::Renderer::Mesh` element: `Position`, `XYZW_Float32`.
    const UNUSED_SLOT: VertexElement =
        VertexElement::new(ElementName::Position, ElementFormat::XYZW_Float32);

    /// Reads a serialized 128-byte description, as `.mapgeo`, `.gmesh` and `.tmesh` store it.
    ///
    /// The reader skips the slots past the element count without reading their contents.
    ///
    /// # Errors
    /// Returns [`ParseError::InvalidField`] for a usage the format does not define, for no
    /// elements or more than [`Self::SERIALIZED_ELEMENT_SLOTS`], for an element name or
    /// format the format does not define, and for an element name that appears twice.
    /// Returns [`ParseError::IOError`] on a short read.
    pub fn from_reader<R: Read + ?Sized>(reader: &mut R) -> crate::Result<Self> {
        let usage_raw = reader.read_u32::<LE>()?;
        let usage = VertexBufferUsage::try_from(usage_raw)
            .map_err(|_| ParseError::InvalidField("vertex buffer usage", usage_raw.to_string()))?;

        let element_count = reader.read_u32::<LE>()?;
        let element_count = usize::try_from(element_count)
            .ok()
            .filter(|count| (1..=Self::SERIALIZED_ELEMENT_SLOTS).contains(count))
            .ok_or_else(|| {
                ParseError::InvalidField("vertex element count", element_count.to_string())
            })?;

        let mut elements = Vec::with_capacity(element_count);
        let mut names = VertexBufferElementFlags::empty();
        for _ in 0..element_count {
            let name_raw = reader.read_u32::<LE>()?;
            let format_raw = reader.read_u32::<LE>()?;
            let name = ElementName::try_from(name_raw).map_err(|_| {
                ParseError::InvalidField("vertex element name", name_raw.to_string())
            })?;
            let format = ElementFormat::try_from(format_raw).map_err(|_| {
                ParseError::InvalidField("vertex element format", format_raw.to_string())
            })?;

            let flag = get_element_flags([name]);
            if names.contains(flag) {
                return Err(ParseError::InvalidField(
                    "vertex element name",
                    format!("{name:?} appears twice"),
                ));
            }
            names |= flag;
            elements.push(VertexElement::new(name, format));
        }

        let unused_bytes = 8 * (Self::SERIALIZED_ELEMENT_SLOTS - element_count) as u64;
        let skipped = io::copy(&mut reader.take(unused_bytes), &mut io::sink())?;
        if skipped != unused_bytes {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
        }

        Ok(Self::new(usage, elements))
    }

    /// Writes the description as a serialized 128-byte description.
    ///
    /// Each slot past the element count holds `Position`, `XYZW_Float32`. The game writes the
    /// same element into an unused slot.
    ///
    /// # Errors
    /// Returns [`ParseError::InvalidField`] for more than
    /// [`Self::SERIALIZED_ELEMENT_SLOTS`] elements, and [`ParseError::IOError`] if the writer
    /// fails.
    pub fn to_writer<W: Write + ?Sized>(&self, writer: &mut W) -> crate::Result<()> {
        let element_count = self.elements.len();
        if element_count > Self::SERIALIZED_ELEMENT_SLOTS {
            return Err(ParseError::InvalidField(
                "vertex element count",
                element_count.to_string(),
            ));
        }

        writer.write_u32::<LE>(self.usage.into())?;
        writer.write_u32::<LE>(element_count as u32)?;
        let unused = std::iter::repeat_n(
            &Self::UNUSED_SLOT,
            Self::SERIALIZED_ELEMENT_SLOTS - element_count,
        );
        for element in self.elements.iter().chain(unused) {
            writer.write_u32::<LE>(element.name.into())?;
            writer.write_u32::<LE>(element.format.into())?;
        }
        Ok(())
    }

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
