//! The rewrite of a version 132 or 134 bank as a version 145 bank.
//!
//! The upgrade rebuilds the header and transforms the payload of each object. The media index
//! keeps its media. It refuses every bank that it cannot convert exactly.

use crate::{
    bank::{BankHeader, BankVersion, Section},
    hierarchy::{
        layout::{Generation, Put, Reader, Step},
        Object, ObjectKind, Typed,
    },
    Bank, Error, Hierarchy, ObjectId, Result,
};

/// The `alignment` of an upgraded header whose legacy `alignment` is 0.
const DEFAULT_ALIGNMENT: u16 = 16;

/// The action classes of the legacy generation without a version 145 counterpart.
const LEGACY_ONLY_CLASSES: [u16; 3] = [0x1500, 0x1600, 0x1700];

/// The offset of the `u16` action type in an action payload, after the object ID.
const ACTION_TYPE_OFFSET: usize = 4;

/// A reason the upgrade does not rewrite a bank.
///
/// [`Bank::upgrade`] returns it as [`Error::Upgrade`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
#[non_exhaustive]
pub enum UpgradeRefusal {
    /// The version is not 132, 134 or 145.
    #[error("bank version {0} is not 132, 134 or 145")]
    Version(BankVersion),
    /// An object of a kind without an upgrade transform.
    #[error("object {id} of kind {kind} has no upgrade transform")]
    Kind {
        /// The ID of the object.
        id: ObjectId,
        /// The kind of the object.
        kind: ObjectKind,
    },
    /// An action of a class without a version 145 counterpart.
    #[error("action {id} of class {class:#06x} has no version 145 counterpart")]
    ActionClass {
        /// The ID of the action.
        id: ObjectId,
        /// The class of the action: `action_type & 0xFF00`.
        class: u16,
    },
}

impl Bank {
    /// Returns the bank rewritten at version 145.
    ///
    /// A version 145 bank upgrades to an equal bank. A version 132 or 134 bank gets a version
    /// 145 header and version 145 payloads. Its media index keeps its media, and the writer
    /// writes it with the alignment of the new header. Every object of the output decodes with the version 145 layouts.
    ///
    /// # Errors
    ///
    /// - [`Error::Upgrade`] when the version is not 132, 134 or 145, an object is of a kind
    ///   without an upgrade transform, or an action is of a class without a version 145
    ///   counterpart.
    /// - [`Error::Layout`] when a payload does not decode at the version of the bank, or the
    ///   upgraded payload does not decode at version 145.
    /// - [`Error::ActionClass`] when an action is of a class outside the table of its version.
    pub fn upgrade(&self) -> Result<Bank> {
        let version = self.header().version;
        if version == BankVersion::V145 {
            return Ok(self.clone());
        }
        if !matches!(version, BankVersion::V132 | BankVersion::V134) {
            return Err(Error::Upgrade(UpgradeRefusal::Version(version)));
        }
        for section in self.sections() {
            if let Section::Hierarchy(hierarchy) = section {
                if let Some(refusal) = hierarchy.objects().iter().find_map(refusal) {
                    return Err(Error::Upgrade(refusal));
                }
            }
        }

        let mut bank = Bank::new(upgrade_header(self.header()));
        for section in self.sections() {
            let section = match section {
                Section::Media(media) => Section::Media(media.clone()),
                Section::Hierarchy(hierarchy) => {
                    Section::Hierarchy(upgrade_hierarchy(hierarchy, version)?)
                }
                Section::Raw(raw) => Section::Raw(raw.clone()),
            };
            bank.sections_mut().push(section);
        }
        Ok(bank)
    }
}

/// Returns the version 145 header of the legacy header `legacy`.
///
/// The header keeps the bank ID, the language ID, the alt values and the project ID. An
/// `alignment` of 0 gives 16. The bank type is 0, the hash is zero and the body has no padding.
fn upgrade_header(legacy: &BankHeader) -> BankHeader {
    let mut header = BankHeader::new(BankVersion::V145, legacy.bank_id);
    header.language_id = legacy.language_id;
    header.alignment = match legacy.alignment {
        0 => DEFAULT_ALIGNMENT,
        alignment => alignment,
    };
    header.alt_values = legacy.alt_values;
    header.project_id = legacy.project_id;
    header
}

/// Returns the refusal of `object`, or `None` when the upgrade transforms it.
fn refusal(object: &Object) -> Option<UpgradeRefusal> {
    let id = object.id();
    match object.kind() {
        ObjectKind::STATE
        | ObjectKind::SOUND
        | ObjectKind::EVENT
        | ObjectKind::RANDOM_SEQUENCE_CONTAINER
        | ObjectKind::SWITCH_CONTAINER
        | ObjectKind::ACTOR_MIXER
        | ObjectKind::LAYER_CONTAINER
        | ObjectKind::ATTENUATION => None,
        ObjectKind::ACTION => {
            let action_type = object
                .payload()
                .get(ACTION_TYPE_OFFSET..)?
                .first_chunk::<2>()?;
            let class = u16::from_le_bytes(*action_type) & 0xFF00;
            LEGACY_ONLY_CLASSES
                .contains(&class)
                .then_some(UpgradeRefusal::ActionClass { id, class })
        }
        kind => Some(UpgradeRefusal::Kind { id, kind }),
    }
}

/// Returns the hierarchy `legacy` of a bank of the version `version` with version 145 payloads.
fn upgrade_hierarchy(legacy: &Hierarchy, version: BankVersion) -> Result<Hierarchy> {
    let mut hierarchy = Hierarchy::new();
    for object in legacy.objects() {
        hierarchy.push(upgrade_object(object, version)?);
    }
    Ok(hierarchy)
}

/// Returns the object `legacy` of a bank of the version `version` with a version 145 payload.
fn upgrade_object(legacy: &Object, version: BankVersion) -> Result<Object> {
    if let Some(refusal) = refusal(legacy) {
        return Err(Error::Upgrade(refusal));
    }
    match legacy.kind() {
        ObjectKind::STATE => Ok(legacy.clone()),
        ObjectKind::ATTENUATION => upgrade_attenuation(legacy),
        ObjectKind::EVENT => {
            legacy.decode(BankVersion::V145)?;
            Ok(legacy.clone())
        }
        _ => typed(legacy, version)?.encode(BankVersion::V145),
    }
}

/// Returns the attenuation `legacy` with a version 145 payload.
///
/// The payload gains a zero byte after the ID, and the curve table widens from 7 to 19 slots of
/// `0xFE`.
fn upgrade_attenuation(legacy: &Object) -> Result<Object> {
    let mut attenuation = Attenuation::decode(legacy, Generation::Legacy)?;
    attenuation
        .curve_slots
        .resize(CURRENT_CURVE_SLOTS, DEFAULT_CURVE);
    let mut payload = Vec::with_capacity(legacy.payload().len() + CURRENT_CURVE_SLOTS);
    attenuation.write(&mut payload, Generation::Current);
    let upgraded = Object::new(ObjectKind::ATTENUATION, payload)?;
    Attenuation::decode(&upgraded, Generation::Current)?;
    Ok(upgraded)
}

/// The curve slots of a legacy attenuation.
const LEGACY_CURVE_SLOTS: usize = 7;

/// The curve slots of a current attenuation.
const CURRENT_CURVE_SLOTS: usize = 19;

/// The curve slot that selects the project default curve in the version 145 reader.
///
/// `0xFF` selects no curve. The version 134 reader applies the project default curves to every
/// attenuation.
const DEFAULT_CURVE: u8 = 0xFE;

/// The length of the cone parameters of an attenuation with the cone on: five `f32`.
const CONE_LEN: usize = 5 * 4;

/// An Attenuation payload (kind 14), with the fields that the upgrade does not change as bytes.
///
/// ```text
/// u32 id
/// current: u8 flag
/// u8 cone_enabled; if cone_enabled: f32[5]
/// u8 curve_slots[7 legacy, 19 current]
/// u8 curve_count; per curve: u8 scaling; u16 point_count; (12 bytes)[point_count]
/// RtpcList
/// ```
struct Attenuation {
    id: ObjectId,
    /// The byte of the current generation after the ID.
    flag: u8,
    /// The cone switch and the cone parameters.
    cone: Vec<u8>,
    /// The curve index of each slot.
    curve_slots: Vec<u8>,
    /// The curves and the RTPC list.
    tail: Vec<u8>,
}

impl Attenuation {
    /// Decodes the attenuation `object` with the layout of `generation`.
    fn decode(object: &Object, generation: Generation) -> Result<Self> {
        let mut reader = Reader::new(object.payload());
        Self::read(&mut reader, generation)
            .and_then(|attenuation| reader.finish().map(|()| attenuation))
            .map_err(|stop| stop.into_error(object.kind(), object.id(), object.payload().len()))
    }

    fn read(reader: &mut Reader<'_>, generation: Generation) -> Step<Self> {
        let id = reader.id()?;
        let flag = if generation.is_current() {
            reader.u8()?
        } else {
            0
        };
        let cone = reader.bytes_of(|reader| match reader.u8()? {
            0 => Ok(()),
            _ => reader.skip(CONE_LEN),
        })?;
        let slots = match generation {
            Generation::Legacy => LEGACY_CURVE_SLOTS,
            Generation::Current => CURRENT_CURVE_SLOTS,
        };
        let curve_slots = reader.take(slots)?.to_vec();
        let tail = reader.bytes_of(|reader| {
            let curves = reader.u8()?;
            for _ in 0..curves {
                reader.skip(1)?;
                let points = reader.u16()?;
                reader.skip_records(u32::from(points), 12)?;
            }
            reader.rtpcs()
        })?;
        Ok(Self {
            id,
            flag,
            cone,
            curve_slots,
            tail,
        })
    }

    fn write(&self, out: &mut Vec<u8>, generation: Generation) {
        out.put_u32(self.id.0);
        if generation.is_current() {
            out.put_u8(self.flag);
        }
        out.extend_from_slice(&self.cone);
        out.extend_from_slice(&self.curve_slots);
        out.extend_from_slice(&self.tail);
    }
}

/// Decodes the object `object` of a kind with a typed layout at `version`.
fn typed(object: &Object, version: BankVersion) -> Result<Typed> {
    object.decode(version)?.ok_or(Error::Layout {
        kind: object.kind(),
        id: object.id(),
        offset: 0,
        len: object.payload().len(),
    })
}
