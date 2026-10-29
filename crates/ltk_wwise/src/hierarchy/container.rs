//! The layouts of the containers: RandomSequenceContainer, SwitchContainer, ActorMixer and
//! LayerContainer.

use crate::{error::to_u16, ObjectId, Result};

use super::{
    layout::{Generation, Put, Reader, Step, Stop},
    NodeBase, ObjectKind, PropValue,
};

/// The length of one RTPC graph point: `f32 from`, `f32 to` and `u32 interp`.
const POINT_LEN: usize = 12;

/// A RandomSequenceContainer, a SwitchContainer, an ActorMixer or a LayerContainer.
///
/// `children` is the child list of the payload. A RandomSequenceContainer, a SwitchContainer and
/// a LayerContainer name children again in `fields`. [`Container::remove_child`] and
/// [`Container::replace_child`] edit both. A direct edit of `children` leaves `fields` as it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Container {
    pub id: ObjectId,
    pub node: NodeBase,
    /// The child objects, in file order.
    pub children: Vec<ObjectId>,
    /// The fields of the kind of the container. The variant selects the kind.
    pub fields: ContainerFields,
}

/// The fields of a container that belong to its kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerFields {
    /// Kind 5.
    RandomSequence(RandomSequenceFields),
    /// Kind 6.
    Switch(SwitchFields),
    /// Kind 7. An ActorMixer has no fields of its own.
    ActorMixer,
    /// Kind 9.
    Layer(LayerFields),
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

    /// Returns the kind of the container, which the variant of `fields` selects.
    pub fn kind(&self) -> ObjectKind {
        match self.fields {
            ContainerFields::RandomSequence(_) => ObjectKind::RANDOM_SEQUENCE_CONTAINER,
            ContainerFields::Switch(_) => ObjectKind::SWITCH_CONTAINER,
            ContainerFields::ActorMixer => ObjectKind::ACTOR_MIXER,
            ContainerFields::Layer(_) => ObjectKind::LAYER_CONTAINER,
        }
    }

    /// Removes `child` from the children and from every list of `fields` that names it.
    ///
    /// A RandomSequenceContainer drops the playlist items of `child`. A SwitchContainer drops
    /// `child` from the node list of each switch and drops its node parameters. A
    /// LayerContainer drops the associations of `child`. A switch or a layer keeps its place when
    /// its list becomes empty.
    ///
    /// Returns `true` when the children held `child`.
    pub fn remove_child(&mut self, child: ObjectId) -> bool {
        let len = self.children.len();
        self.children.retain(|&id| id != child);
        match &mut self.fields {
            ContainerFields::RandomSequence(fields) => {
                fields.playlist.retain(|item| item.child != child);
            }
            ContainerFields::Switch(fields) => {
                for switch in &mut fields.switches {
                    switch.nodes.retain(|&node| node != child);
                }
                fields.params.retain(|params| params.node != child);
            }
            ContainerFields::ActorMixer => {}
            ContainerFields::Layer(fields) => {
                for layer in &mut fields.layers {
                    layer
                        .associations
                        .retain(|association| association.child != child);
                }
            }
        }
        self.children.len() != len
    }

    /// Replaces `old` with `new` in the children and in every list of `fields` that names it.
    ///
    /// Each ID keeps its place.
    ///
    /// Returns `true` when the children held `old`.
    pub fn replace_child(&mut self, old: ObjectId, new: ObjectId) -> bool {
        let rename = |id: &mut ObjectId| {
            if *id == old {
                *id = new;
            }
        };
        let held = self.children.contains(&old);
        self.children.iter_mut().for_each(rename);
        match &mut self.fields {
            ContainerFields::RandomSequence(fields) => {
                for item in &mut fields.playlist {
                    rename(&mut item.child);
                }
            }
            ContainerFields::Switch(fields) => {
                for switch in &mut fields.switches {
                    switch.nodes.iter_mut().for_each(rename);
                }
                for params in &mut fields.params {
                    rename(&mut params.node);
                }
            }
            ContainerFields::ActorMixer => {}
            ContainerFields::Layer(fields) => {
                for layer in &mut fields.layers {
                    for association in &mut layer.associations {
                        rename(&mut association.child);
                    }
                }
            }
        }
        held
    }

    /// Reads a container payload of the kind `kind`.
    ///
    /// Stops at offset 0 for a kind other than 5, 6, 7 and 9.
    pub(super) fn read(
        reader: &mut Reader<'_>,
        kind: ObjectKind,
        generation: Generation,
    ) -> Step<Self> {
        let id = reader.id()?;
        let node = NodeBase::read(reader, generation)?;
        let mut fields = match kind {
            ObjectKind::RANDOM_SEQUENCE_CONTAINER => {
                ContainerFields::RandomSequence(RandomSequenceFields::read_head(reader)?)
            }
            ObjectKind::SWITCH_CONTAINER => {
                ContainerFields::Switch(SwitchFields::read_head(reader)?)
            }
            ObjectKind::ACTOR_MIXER => ContainerFields::ActorMixer,
            ObjectKind::LAYER_CONTAINER => ContainerFields::Layer(LayerFields::default()),
            _ => return Err(Stop::At(0)),
        };
        let count = reader.u32()?;
        let children = reader.ids(count)?;
        match &mut fields {
            ContainerFields::RandomSequence(fields) => fields.read_tail(reader)?,
            ContainerFields::Switch(fields) => fields.read_tail(reader)?,
            ContainerFields::ActorMixer => {}
            ContainerFields::Layer(fields) => fields.read_tail(reader)?,
        }
        Ok(Self {
            id,
            node,
            children,
            fields,
        })
    }

    /// Writes a container payload with the layout of `generation`.
    pub(super) fn write(&self, out: &mut Vec<u8>, generation: Generation) -> Result<()> {
        out.put_u32(self.id.0);
        self.node.write(out, generation)?;
        match &self.fields {
            ContainerFields::RandomSequence(fields) => fields.write_head(out),
            ContainerFields::Switch(fields) => fields.write_head(out),
            ContainerFields::ActorMixer | ContainerFields::Layer(_) => {}
        }
        out.put_count(self.children.len(), "child count")?;
        out.put_ids(&self.children);
        match &self.fields {
            ContainerFields::RandomSequence(fields) => fields.write_tail(out),
            ContainerFields::Switch(fields) => fields.write_tail(out),
            ContainerFields::ActorMixer => Ok(()),
            ContainerFields::Layer(fields) => fields.write_tail(out),
        }
    }
}

/// The play mode of a RandomSequenceContainer: `eMode`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContainerMode(pub u8);

impl ContainerMode {
    /// Plays a child that the client picks by weight: 0.
    pub const RANDOM: Self = Self(0);
    /// Plays the children in playlist order: 1.
    pub const SEQUENCE: Self = Self(1);
}

/// The fields of a RandomSequenceContainer.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RandomSequenceFields {
    /// The loop count. Zero loops forever. `sLoopCount`.
    pub loop_count: u16,
    /// The lower bound of the random offset of the loop count. `sLoopModMin`.
    pub loop_mod_min: u16,
    /// The upper bound of the random offset of the loop count. `sLoopModMax`.
    pub loop_mod_max: u16,
    /// The transition time in seconds, the bits of an `f32`. `fTransitionTime`.
    pub transition_time: PropValue,
    /// The lower bound of the random offset of the transition time, the bits of an `f32`.
    pub transition_time_mod_min: PropValue,
    /// The upper bound of the random offset of the transition time, the bits of an `f32`.
    pub transition_time_mod_max: PropValue,
    /// The number of children played before a child repeats. `wAvoidRepeatCount`.
    pub avoid_repeat_count: u16,
    /// `eTransitionMode`: 0 none, 1 crossfade amplitude, 2 crossfade power, 3 delay, 4 sample
    /// accurate, 5 trigger rate.
    pub transition_mode: u8,
    /// `eRandomMode`: 0 normal, 1 shuffle.
    pub random_mode: u8,
    pub mode: ContainerMode,
    /// Bit 0 uses weights, bit 1 resets the playlist at each play, bit 2 restarts backward, bit
    /// 3 is continuous, bit 4 is global.
    pub bits: u8,
    /// The playlist, in play order for a sequence.
    pub playlist: Vec<PlaylistItem>,
}

/// One item of the playlist of a RandomSequenceContainer: `AkPlaylistItem`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlaylistItem {
    pub child: ObjectId,
    /// The weight of the child in a random pick, relative to the other items.
    pub weight: u32,
}

impl RandomSequenceFields {
    /// Reads the fields between the node base and the children.
    fn read_head(reader: &mut Reader<'_>) -> Step<Self> {
        Ok(Self {
            loop_count: reader.u16()?,
            loop_mod_min: reader.u16()?,
            loop_mod_max: reader.u16()?,
            transition_time: PropValue(reader.u32()?),
            transition_time_mod_min: PropValue(reader.u32()?),
            transition_time_mod_max: PropValue(reader.u32()?),
            avoid_repeat_count: reader.u16()?,
            transition_mode: reader.u8()?,
            random_mode: reader.u8()?,
            mode: ContainerMode(reader.u8()?),
            bits: reader.u8()?,
            playlist: Vec::new(),
        })
    }

    /// Reads the playlist: a `u16` count of `(u32 child, u32 weight)` items.
    fn read_tail(&mut self, reader: &mut Reader<'_>) -> Step<()> {
        let count = reader.u16()?;
        self.playlist = (0..count)
            .map(|_| {
                Ok(PlaylistItem {
                    child: reader.id()?,
                    weight: reader.u32()?,
                })
            })
            .collect::<Step<_>>()?;
        Ok(())
    }

    fn write_head(&self, out: &mut Vec<u8>) {
        out.put_u16(self.loop_count);
        out.put_u16(self.loop_mod_min);
        out.put_u16(self.loop_mod_max);
        out.put_u32(self.transition_time.0);
        out.put_u32(self.transition_time_mod_min.0);
        out.put_u32(self.transition_time_mod_max.0);
        out.put_u16(self.avoid_repeat_count);
        out.put_u8(self.transition_mode);
        out.put_u8(self.random_mode);
        out.put_u8(self.mode.0);
        out.put_u8(self.bits);
    }

    fn write_tail(&self, out: &mut Vec<u8>) -> Result<()> {
        out.put_u16(to_u16(self.playlist.len(), "playlist count")?);
        for item in &self.playlist {
            out.put_u32(item.child.0);
            out.put_u32(item.weight);
        }
        Ok(())
    }
}

/// The kind of the group of a SwitchContainer: `eGroupType`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GroupType(pub u8);

impl GroupType {
    /// A switch group: 0.
    pub const SWITCH: Self = Self(0);
    /// A state group: 1.
    pub const STATE: Self = Self(1);
}

/// The fields of a SwitchContainer.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SwitchFields {
    pub group_type: GroupType,
    /// The ID of the switch group or the state group: the FNV-1 hash of its name.
    pub group: u32,
    /// The switch that selects the nodes when the game sets no switch of the group.
    pub default_switch: u32,
    /// `bIsContinuousValidation`. Nonzero is on.
    pub continuous_validation: u8,
    /// The nodes of each switch, in file order.
    pub switches: Vec<SwitchNodes>,
    /// The parameters of each node, in file order.
    pub params: Vec<SwitchNodeParams>,
}

/// The nodes that one switch of a SwitchContainer plays: `CAkSwitchPackage`.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct SwitchNodes {
    /// The ID of the switch or the state: the FNV-1 hash of its name.
    pub switch: u32,
    pub nodes: Vec<ObjectId>,
}

/// The parameters of one node of a SwitchContainer: `AkSwitchNodeParams`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SwitchNodeParams {
    pub node: ObjectId,
    /// Bit 0 plays the node on the first switch only, bit 1 continues playback across a switch.
    pub bits: u8,
    /// Bits 0 to 2 are the on-switch mode: 0 plays to the end, 1 stops.
    pub on_switch_bits: u8,
    /// The fade-out time in milliseconds.
    pub fade_out_time: i32,
    /// The fade-in time in milliseconds.
    pub fade_in_time: i32,
}

impl SwitchFields {
    /// Returns the nodes of the first entry of the switch `switch`, or `None` when no entry
    /// names it.
    pub fn nodes_of(&self, switch: u32) -> Option<&[ObjectId]> {
        self.switches
            .iter()
            .find(|entry| entry.switch == switch)
            .map(|entry| &entry.nodes[..])
    }

    /// Reads the fields between the node base and the children.
    fn read_head(reader: &mut Reader<'_>) -> Step<Self> {
        Ok(Self {
            group_type: GroupType(reader.u8()?),
            group: reader.u32()?,
            default_switch: reader.u32()?,
            continuous_validation: reader.u8()?,
            switches: Vec::new(),
            params: Vec::new(),
        })
    }

    /// Reads the switches and the node parameters.
    fn read_tail(&mut self, reader: &mut Reader<'_>) -> Step<()> {
        let count = reader.u32()?;
        self.switches = (0..count)
            .map(|_| {
                let switch = reader.u32()?;
                let nodes = reader.u32()?;
                Ok(SwitchNodes {
                    switch,
                    nodes: reader.ids(nodes)?,
                })
            })
            .collect::<Step<_>>()?;
        let count = reader.u32()?;
        self.params = (0..count)
            .map(|_| {
                Ok(SwitchNodeParams {
                    node: reader.id()?,
                    bits: reader.u8()?,
                    on_switch_bits: reader.u8()?,
                    fade_out_time: reader.i32()?,
                    fade_in_time: reader.i32()?,
                })
            })
            .collect::<Step<_>>()?;
        Ok(())
    }

    fn write_head(&self, out: &mut Vec<u8>) {
        out.put_u8(self.group_type.0);
        out.put_u32(self.group);
        out.put_u32(self.default_switch);
        out.put_u8(self.continuous_validation);
    }

    fn write_tail(&self, out: &mut Vec<u8>) -> Result<()> {
        out.put_count(self.switches.len(), "switch count")?;
        for switch in &self.switches {
            out.put_u32(switch.switch);
            out.put_count(switch.nodes.len(), "node count")?;
            out.put_ids(&switch.nodes);
        }
        out.put_count(self.params.len(), "switch parameter count")?;
        for params in &self.params {
            out.put_u32(params.node.0);
            out.put_u8(params.bits);
            out.put_u8(params.on_switch_bits);
            out.put_i32(params.fade_out_time);
            out.put_i32(params.fade_in_time);
        }
        Ok(())
    }
}

/// The fields of a LayerContainer.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LayerFields {
    pub layers: Vec<Layer>,
    /// `bIsContinuousValidation`. Nonzero is on.
    pub continuous_validation: u8,
}

/// One layer of a LayerContainer: `CAkLayer`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer {
    pub id: u32,
    /// The `RtpcList` of the layer.
    rtpcs: Vec<u8>,
    /// The game parameter of the crossfade: `rtpcID`.
    pub rtpc: u32,
    /// The type of `rtpc`: `rtpcType`.
    pub rtpc_type: u8,
    /// The children that the layer plays, each with its crossfade curve.
    pub associations: Vec<LayerAssociation>,
}

/// One child of a layer and its crossfade curve: `CAssociatedChildData`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerAssociation {
    pub child: ObjectId,
    /// The graph points of the curve, 12 bytes each.
    curve: Vec<u8>,
}

impl LayerFields {
    /// Reads the layers and the `continuous_validation` byte.
    fn read_tail(&mut self, reader: &mut Reader<'_>) -> Step<()> {
        let count = reader.u32()?;
        self.layers = (0..count)
            .map(|_| Layer::read(reader))
            .collect::<Step<_>>()?;
        self.continuous_validation = reader.u8()?;
        Ok(())
    }

    fn write_tail(&self, out: &mut Vec<u8>) -> Result<()> {
        out.put_count(self.layers.len(), "layer count")?;
        for layer in &self.layers {
            layer.write(out)?;
        }
        out.put_u8(self.continuous_validation);
        Ok(())
    }
}

impl Layer {
    fn read(reader: &mut Reader<'_>) -> Step<Self> {
        let id = reader.u32()?;
        let rtpcs = reader.bytes_of(Reader::rtpcs)?;
        let rtpc = reader.u32()?;
        let rtpc_type = reader.u8()?;
        let count = reader.u32()?;
        let associations = (0..count)
            .map(|_| {
                let child = reader.id()?;
                let points = reader.u32()?;
                let curve = reader
                    .take((points as usize).saturating_mul(POINT_LEN))?
                    .to_vec();
                Ok(LayerAssociation { child, curve })
            })
            .collect::<Step<_>>()?;
        Ok(Self {
            id,
            rtpcs,
            rtpc,
            rtpc_type,
            associations,
        })
    }

    fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        out.put_u32(self.id);
        out.extend_from_slice(&self.rtpcs);
        out.put_u32(self.rtpc);
        out.put_u8(self.rtpc_type);
        out.put_count(self.associations.len(), "association count")?;
        for association in &self.associations {
            out.put_u32(association.child.0);
            out.put_count(association.curve.len() / POINT_LEN, "curve point count")?;
            out.extend_from_slice(&association.curve);
        }
        Ok(())
    }
}
