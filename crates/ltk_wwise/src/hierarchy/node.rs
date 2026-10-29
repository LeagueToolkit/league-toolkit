//! The node base: the parameter block of a Sound, a container and an ActorMixer.

use crate::{ObjectId, Result};

use super::{
    layout::{Generation, Put, Reader, Step},
    props::{read_bundle, write_bundle},
    PropBundle, RangedPropBundle,
};

/// The metadata block of the current generation with no metadata: no override, no entries.
const NO_METADATA: [u8; 2] = [0, 0];

/// The node base of a Sound, a container or an ActorMixer.
///
/// The fields that the crate does not name are kept as bytes. The current generation inserts a
/// metadata block after the FX block and a reflections aux bus after the aux block. A node base
/// of the legacy generation encodes them as zero at the current generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeBase {
    /// The output bus that overrides the bus of the parent, or zero.
    pub override_bus: ObjectId,
    /// The parent in the actor-mixer or the container hierarchy, or zero.
    pub parent: ObjectId,
    /// `override_parent_fx`, `fx_count` and the FX entries.
    fx: Vec<u8>,
    /// `override_parent_metadata`, `metadata_count` and the metadata entries. Empty at the
    /// legacy generation.
    metadata: Vec<u8>,
    /// `override_attachment_params`.
    attachment: u8,
    /// The `bits` before the property bundles.
    bits: u8,
    /// The properties of the node.
    pub props: PropBundle,
    /// The randomized properties of the node.
    pub ranged_props: RangedPropBundle,
    /// The `Positioning` block and the aux block.
    positioning: Vec<u8>,
    /// The reflections aux bus of the current generation. Zero at the legacy generation.
    reflections_aux_bus: u32,
    /// The advanced settings, the `StateChunk` and the `RtpcList`.
    tail: Vec<u8>,
}

impl NodeBase {
    /// Reads a node base.
    pub(super) fn read(reader: &mut Reader<'_>, generation: Generation) -> Step<Self> {
        let fx = reader.bytes_of(|reader| {
            reader.skip(1)?;
            let count = reader.u8()?;
            if count != 0 {
                reader.skip(1)?;
                reader.skip_records(u32::from(count), 1 + 4 + 1 + 1)?;
            }
            Ok(())
        })?;
        let metadata = if generation.is_current() {
            reader.bytes_of(|reader| {
                reader.skip(1)?;
                let count = reader.u8()?;
                reader.skip_records(u32::from(count), 1 + 4 + 1)
            })?
        } else {
            Vec::new()
        };
        let attachment = reader.u8()?;
        let override_bus = reader.id()?;
        let parent = reader.id()?;
        let bits = reader.u8()?;
        let props = read_bundle(reader)?;
        let ranged_props = read_bundle(reader)?;
        let positioning = reader.bytes_of(|reader| {
            reader.positioning()?;
            let aux_bits = reader.u8()?;
            if aux_bits & 0b1000 != 0 {
                reader.skip(4 * 4)?;
            }
            Ok(())
        })?;
        let reflections_aux_bus = if generation.is_current() {
            reader.u32()?
        } else {
            0
        };
        let tail = reader.bytes_of(|reader| {
            reader.skip(1 + 1 + 2 + 1 + 1)?;
            reader.state_chunk()?;
            reader.rtpcs()
        })?;
        Ok(Self {
            override_bus,
            parent,
            fx,
            metadata,
            attachment,
            bits,
            props,
            ranged_props,
            positioning,
            reflections_aux_bus,
            tail,
        })
    }

    /// Writes the node base with the layout of `generation`.
    pub(super) fn write(&self, out: &mut Vec<u8>, generation: Generation) -> Result<()> {
        out.extend_from_slice(&self.fx);
        if generation.is_current() {
            if self.metadata.is_empty() {
                out.extend_from_slice(&NO_METADATA);
            } else {
                out.extend_from_slice(&self.metadata);
            }
        }
        out.put_u8(self.attachment);
        out.put_u32(self.override_bus.0);
        out.put_u32(self.parent.0);
        out.put_u8(self.bits);
        write_bundle(&self.props, out)?;
        write_bundle(&self.ranged_props, out)?;
        out.extend_from_slice(&self.positioning);
        if generation.is_current() {
            out.put_u32(self.reflections_aux_bus);
        }
        out.extend_from_slice(&self.tail);
        Ok(())
    }
}
