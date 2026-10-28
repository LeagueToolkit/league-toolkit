//! The layouts of the containers: RandomSequenceContainer, SwitchContainer, ActorMixer and
//! LayerContainer.

use crate::{ObjectId, Result};

use super::{
    layout::{Generation, Put, Reader, Step},
    NodeBase, ObjectKind,
};

/// A RandomSequenceContainer, a SwitchContainer, an ActorMixer or a LayerContainer.
///
/// A SwitchContainer and a LayerContainer name children again in their groups and layers, in the
/// fields kept as bytes. An edit of `children` does not change those lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Container {
    pub id: ObjectId,
    /// The kind of the container. It selects the layout of the fields kept as bytes.
    pub kind: ObjectKind,
    pub node: NodeBase,
    /// The child objects, in file order.
    pub children: Vec<ObjectId>,
    /// The fields between the node base and the children.
    before_children: Vec<u8>,
    /// The fields after the children.
    after_children: Vec<u8>,
}

impl Container {
    /// Returns `true` for the kinds of the containers.
    pub(super) fn is_container(kind: ObjectKind) -> bool {
        matches!(
            kind,
            ObjectKind::RANDOM_SEQUENCE_CONTAINER
                | ObjectKind::SWITCH_CONTAINER
                | ObjectKind::ACTOR_MIXER
                | ObjectKind::LAYER_CONTAINER
        )
    }

    /// Reads a container payload of the kind `kind`, a container kind.
    pub(super) fn read(
        reader: &mut Reader<'_>,
        kind: ObjectKind,
        generation: Generation,
    ) -> Step<Self> {
        let id = reader.id()?;
        let node = NodeBase::read(reader, generation)?;
        let before_children = reader.bytes_of(|reader| match kind {
            ObjectKind::RANDOM_SEQUENCE_CONTAINER => reader.skip(3 * 2 + 3 * 4 + 2 + 4),
            ObjectKind::SWITCH_CONTAINER => reader.skip(1 + 4 + 4 + 1),
            _ => Ok(()),
        })?;
        let count = reader.u32()?;
        let children = reader.ids(count)?;
        let after_children = reader.bytes_of(|reader| match kind {
            ObjectKind::RANDOM_SEQUENCE_CONTAINER => {
                let count = reader.u16()?;
                reader.skip_records(u32::from(count), 4 + 4)
            }
            ObjectKind::SWITCH_CONTAINER => read_switch_lists(reader),
            ObjectKind::LAYER_CONTAINER => read_layers(reader),
            _ => Ok(()),
        })?;
        Ok(Self {
            id,
            kind,
            node,
            children,
            before_children,
            after_children,
        })
    }

    /// Writes a container payload with the layout of `generation`.
    pub(super) fn write(&self, out: &mut Vec<u8>, generation: Generation) -> Result<()> {
        out.put_u32(self.id.0);
        self.node.write(out, generation);
        out.extend_from_slice(&self.before_children);
        out.put_count(self.children.len(), "child count")?;
        out.put_ids(&self.children);
        out.extend_from_slice(&self.after_children);
        Ok(())
    }
}

/// Skips the switch groups and the switch parameters of a SwitchContainer.
fn read_switch_lists(reader: &mut Reader<'_>) -> Step<()> {
    let groups = reader.u32()?;
    for _ in 0..groups {
        reader.skip(4)?;
        let nodes = reader.u32()?;
        reader.skip_records(nodes, 4)?;
    }
    let params = reader.u32()?;
    reader.skip_records(params, 4 + 1 + 1 + 4 + 4)
}

/// Skips the layers and the `continuous_validation` byte of a LayerContainer.
fn read_layers(reader: &mut Reader<'_>) -> Step<()> {
    let layers = reader.u32()?;
    for _ in 0..layers {
        reader.skip(4)?;
        reader.rtpcs()?;
        reader.skip(4 + 1)?;
        let associations = reader.u32()?;
        for _ in 0..associations {
            reader.skip(4)?;
            let points = reader.u32()?;
            reader.skip_records(points, 12)?;
        }
    }
    reader.skip(1)
}
