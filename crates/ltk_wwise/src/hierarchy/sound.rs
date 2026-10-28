//! The Sound layout and its `BankSource`.

use crate::{ObjectId, Result, SourceId};

use super::{
    layout::{Generation, Put, Reader, Step},
    to_u32, NodeBase,
};

/// The plugin type of a source plugin in the low nibble of a plugin ID.
const SOURCE_PLUGIN_TYPE: u32 = 2;

/// A sound: one media source and its node base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sound {
    pub id: ObjectId,
    pub source: BankSource,
    pub node: NodeBase,
}

impl Sound {
    /// Reads a sound payload.
    pub(super) fn read(reader: &mut Reader<'_>, generation: Generation) -> Step<Self> {
        let id = reader.id()?;
        let source = BankSource::read(reader)?;
        let node = NodeBase::read(reader, generation)?;
        Ok(Self { id, source, node })
    }

    /// Writes a sound payload with the layout of `generation`.
    pub(super) fn write(&self, out: &mut Vec<u8>, generation: Generation) -> Result<()> {
        out.put_u32(self.id.0);
        self.source.write(out)?;
        self.node.write(out, generation);
        Ok(())
    }
}

/// The media source of a sound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankSource {
    /// The codec or the source plugin. A low nibble of 2 marks a source plugin.
    pub plugin_id: u32,
    pub stream_type: StreamType,
    pub source_id: SourceId,
    /// The length of the media in a bank, or of its prefetched start.
    pub in_memory_size: u32,
    pub source_bits: u8,
    /// The parameters of a source plugin. Empty for a codec.
    params: Vec<u8>,
}

impl BankSource {
    /// Returns `true` when the plugin ID names a source plugin, which carries parameters.
    fn has_params(&self) -> bool {
        self.plugin_id & 0x0F == SOURCE_PLUGIN_TYPE
    }

    /// Reads a `BankSource`.
    fn read(reader: &mut Reader<'_>) -> Step<Self> {
        let mut source = Self {
            plugin_id: reader.u32()?,
            stream_type: StreamType(reader.u8()?),
            source_id: SourceId(reader.u32()?),
            in_memory_size: reader.u32()?,
            source_bits: reader.u8()?,
            params: Vec::new(),
        };
        if source.has_params() {
            let len = reader.u32()?;
            source.params = reader.take(len as usize)?.to_vec();
        }
        Ok(source)
    }

    /// Writes a `BankSource`.
    fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        out.put_u32(self.plugin_id);
        out.put_u8(self.stream_type.0);
        out.put_u32(self.source_id.0);
        out.put_u32(self.in_memory_size);
        out.put_u8(self.source_bits);
        if self.has_params() {
            out.put_u32(to_u32(self.params.len(), "plugin parameters")?);
            out.extend_from_slice(&self.params);
        }
        Ok(())
    }
}

/// Where the media of a source is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StreamType(pub u8);

impl StreamType {
    /// The media is in the `DATA` of a loaded bank.
    pub const DATA_BANK: Self = Self(0);
    /// The start of the media is in a bank. The rest streams from a package.
    pub const PREFETCH: Self = Self(1);
    /// The media streams from a package.
    pub const STREAMING: Self = Self(2);
}
