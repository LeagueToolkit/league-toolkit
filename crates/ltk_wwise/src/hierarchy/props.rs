//! The property bundles: `PropBundle` and `RangedPropBundle`.

use std::{fmt, slice};

use crate::{error::to_u8, Result};

use super::layout::{Put, Reader, Step};

/// The ID of a property in a property bundle: `AkPropID`.
///
/// Bank versions 132, 134 and 145 share one table of IDs. The constants name the IDs that shipped
/// objects hold. [`PropertyId::value_type`] gives the type of the value of a named ID.
///
/// ```
/// use ltk_wwise::hierarchy::{PropertyId, ValueType};
///
/// assert_eq!(PropertyId::PITCH.to_string(), "Pitch");
/// assert_eq!(PropertyId::LOOP.value_type(), Some(ValueType::I32));
/// assert_eq!(PropertyId(0xF0).to_string(), "prop(0xf0)");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropertyId(pub u8);

/// Declares the named property IDs: a constant, a name and a value type each.
macro_rules! properties {
    ($($(#[$doc:meta])* $name:ident = $id:literal, $text:literal, $ty:ident;)*) => {
        impl PropertyId {
            $($(#[$doc])* pub const $name: Self = Self($id);)*

            /// Returns the Wwise name of the ID, or `None` for an ID that the crate does not name.
            pub fn name(self) -> Option<&'static str> {
                match self {
                    $(Self::$name => Some($text),)*
                    _ => None,
                }
            }

            /// Returns the type of the value of the ID, or `None` for an ID that the crate does
            /// not name.
            pub fn value_type(self) -> Option<ValueType> {
                match self {
                    $(Self::$name => Some(ValueType::$ty),)*
                    _ => None,
                }
            }
        }
    };
}

properties! {
    /// The volume in decibels: 0x00.
    VOLUME = 0x00, "Volume", F32;
    /// The pitch in cents: 0x02.
    PITCH = 0x02, "Pitch", F32;
    /// The low-pass filter, 0 to 100: 0x03.
    LPF = 0x03, "LPF", F32;
    /// The high-pass filter, 0 to 100: 0x04.
    HPF = 0x04, "HPF", F32;
    /// The make-up gain in decibels: 0x06.
    MAKE_UP_GAIN = 0x06, "MakeUpGain", F32;
    /// The playback priority, 0 to 100: 0x07.
    PRIORITY = 0x07, "Priority", F32;
    /// The offset of the priority at the maximum distance: 0x08.
    PRIORITY_DISTANCE_OFFSET = 0x08, "PriorityDistanceOffset", F32;
    /// The left-right pan, -100 to 100: 0x0C.
    PAN_LR = 0x0C, "PAN_LR", F32;
    /// The front-rear pan, -100 to 100: 0x0D.
    PAN_FR = 0x0D, "PAN_FR", F32;
    /// The center percentage, 0 to 100: 0x0E.
    CENTER_PCT = 0x0E, "CenterPCT", F32;
    /// The delay of an action in milliseconds: 0x0F.
    DELAY_TIME = 0x0F, "DelayTime", I32;
    /// The fade time of an action in milliseconds: 0x10.
    TRANSITION_TIME = 0x10, "TransitionTime", I32;
    /// The probability of a Play action in percent: 0x11.
    PROBABILITY = 0x11, "Probability", F32;
    /// The send volume of the first user aux bus in decibels: 0x13.
    USER_AUX_SEND_VOLUME_0 = 0x13, "UserAuxSendVolume0", F32;
    /// The output bus volume in decibels: 0x18.
    OUTPUT_BUS_VOLUME = 0x18, "OutputBusVolume", F32;
    /// The HDR active range in decibels: 0x21.
    HDR_ACTIVE_RANGE = 0x21, "HDRActiveRange", F32;
    /// The loop count. Zero loops forever: 0x3A.
    LOOP = 0x3A, "Loop", I32;
    /// The delay of a node in seconds: 0x3B.
    INITIAL_DELAY = 0x3B, "InitialDelay", F32;
    /// The ID of the Attenuation object of a node: 0x46.
    ATTENUATION_ID = 0x46, "AttenuationID", U32;
    /// The blend of 2D and 3D positioning, 0 to 100: 0x47.
    POSITIONING_TYPE_BLEND = 0x47, "PositioningTypeBlend", F32;
}

/// Writes the Wwise name of the ID, or `prop(0xNN)` for an ID that the crate does not name.
impl fmt::Display for PropertyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => f.pad(name),
            None => f.pad(&format!("prop({:#04x})", self.0)),
        }
    }
}

/// The type of the value of a property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueType {
    /// An `f32`.
    F32,
    /// An `i32`: a count or a time in milliseconds.
    I32,
    /// A `u32`: an object ID.
    U32,
}

/// The value of a property: the bits of an `f32`, an `i32` or a `u32`.
///
/// The type of the value belongs to the [`PropertyId`]. Equality compares the bits.
///
/// ```
/// use ltk_wwise::hierarchy::PropValue;
///
/// assert_eq!(PropValue::from_f32(1.5).to_f32(), 1.5);
/// assert_eq!(PropValue::from_i32(-100).0, 0xFFFF_FF9C);
/// assert_eq!(PropValue(7).0, 7);
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PropValue(pub u32);

impl PropValue {
    /// Returns the value with the bits of `value`.
    pub fn from_f32(value: f32) -> Self {
        Self(value.to_bits())
    }

    /// Returns the bits read as an `f32`.
    pub fn to_f32(self) -> f32 {
        f32::from_bits(self.0)
    }

    /// Returns the value with the bits of `value`.
    pub fn from_i32(value: i32) -> Self {
        Self(u32::from_ne_bytes(value.to_ne_bytes()))
    }

    /// Returns the bits read as an `i32`.
    pub fn to_i32(self) -> i32 {
        i32::from_ne_bytes(self.0.to_ne_bytes())
    }
}

impl From<u32> for PropValue {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

/// The bounds of a randomized property: `RANGED_MODIFIERS`.
///
/// The client offsets the value of the property by a random amount between `min` and `max`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PropRange {
    pub min: PropValue,
    pub max: PropValue,
}

/// One property of a bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Prop<V = PropValue> {
    pub id: PropertyId,
    pub value: V,
}

/// A property bundle: a list of property IDs, each with a value of the type `V`.
///
/// The list keeps the order of the payload. An ID occurs at most once in a bundle that the client
/// writes. [`PropBundle::get`], [`PropBundle::set`] and [`PropBundle::remove`] act on the first
/// property of an ID.
///
/// ```
/// use ltk_wwise::hierarchy::{PropBundle, PropValue, PropertyId};
///
/// let mut props = PropBundle::new();
/// props.set(PropertyId::VOLUME, PropValue::from_f32(-3.0));
/// assert_eq!(props.f32(PropertyId::VOLUME), Some(-3.0));
/// assert_eq!(props.u32(PropertyId::VOLUME), None);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PropBundle<V = PropValue> {
    props: Vec<Prop<V>>,
}

/// A `RangedPropBundle`: a property bundle of ranges.
pub type RangedPropBundle = PropBundle<PropRange>;

impl<V> Default for PropBundle<V> {
    fn default() -> Self {
        Self { props: Vec::new() }
    }
}

impl<V: Copy> PropBundle<V> {
    /// Returns a bundle with no properties.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of properties.
    pub fn len(&self) -> usize {
        self.props.len()
    }

    /// Returns `true` when the bundle holds no properties.
    pub fn is_empty(&self) -> bool {
        self.props.is_empty()
    }

    /// Returns the properties, in payload order.
    pub fn iter(&self) -> slice::Iter<'_, Prop<V>> {
        self.props.iter()
    }

    /// Returns the value of the first property with the ID `id`.
    pub fn get(&self, id: PropertyId) -> Option<V> {
        self.props.iter().find(|p| p.id == id).map(|p| p.value)
    }

    /// Sets the value of the first property with the ID `id`, or appends the property.
    ///
    /// Returns the replaced value, or `None` when no property has the ID.
    pub fn set(&mut self, id: PropertyId, value: V) -> Option<V> {
        match self.props.iter_mut().find(|p| p.id == id) {
            Some(prop) => Some(std::mem::replace(&mut prop.value, value)),
            None => {
                self.props.push(Prop { id, value });
                None
            }
        }
    }

    /// Removes the first property with the ID `id`.
    ///
    /// The other properties keep their order.
    pub fn remove(&mut self, id: PropertyId) -> Option<V> {
        let index = self.props.iter().position(|p| p.id == id)?;
        Some(self.props.remove(index).value)
    }

    /// Returns the value of `id` read as `ty`, or `None` when the ID has another type.
    fn typed(&self, id: PropertyId, ty: ValueType) -> Option<V> {
        (id.value_type() == Some(ty)).then(|| self.get(id))?
    }
}

impl PropBundle {
    /// Returns the value of `id` as an `f32`.
    ///
    /// Returns `None` when no property has the ID, or when the type of the ID is not
    /// [`ValueType::F32`].
    pub fn f32(&self, id: PropertyId) -> Option<f32> {
        self.typed(id, ValueType::F32).map(PropValue::to_f32)
    }

    /// Returns the value of `id` as an `i32`.
    ///
    /// Returns `None` when no property has the ID, or when the type of the ID is not
    /// [`ValueType::I32`].
    pub fn i32(&self, id: PropertyId) -> Option<i32> {
        self.typed(id, ValueType::I32).map(PropValue::to_i32)
    }

    /// Returns the value of `id` as a `u32`.
    ///
    /// Returns `None` when no property has the ID, or when the type of the ID is not
    /// [`ValueType::U32`].
    pub fn u32(&self, id: PropertyId) -> Option<u32> {
        self.typed(id, ValueType::U32).map(|value| value.0)
    }
}

impl RangedPropBundle {
    /// Returns the bounds of `id` as `f32`s.
    ///
    /// Returns `None` when no property has the ID, or when the type of the ID is not
    /// [`ValueType::F32`].
    pub fn f32(&self, id: PropertyId) -> Option<(f32, f32)> {
        self.typed(id, ValueType::F32)
            .map(|range| (range.min.to_f32(), range.max.to_f32()))
    }

    /// Returns the bounds of `id` as `i32`s.
    ///
    /// Returns `None` when no property has the ID, or when the type of the ID is not
    /// [`ValueType::I32`].
    pub fn i32(&self, id: PropertyId) -> Option<(i32, i32)> {
        self.typed(id, ValueType::I32)
            .map(|range| (range.min.to_i32(), range.max.to_i32()))
    }

    /// Returns the bounds of `id` as `u32`s.
    ///
    /// Returns `None` when no property has the ID, or when the type of the ID is not
    /// [`ValueType::U32`].
    pub fn u32(&self, id: PropertyId) -> Option<(u32, u32)> {
        self.typed(id, ValueType::U32)
            .map(|range| (range.min.0, range.max.0))
    }
}

impl<V> FromIterator<Prop<V>> for PropBundle<V> {
    fn from_iter<I: IntoIterator<Item = Prop<V>>>(iter: I) -> Self {
        Self {
            props: iter.into_iter().collect(),
        }
    }
}

impl<'a, V> IntoIterator for &'a PropBundle<V> {
    type Item = &'a Prop<V>;
    type IntoIter = slice::Iter<'a, Prop<V>>;

    fn into_iter(self) -> Self::IntoIter {
        self.props.iter()
    }
}

/// The wire form of the value of a bundle.
pub(super) trait WireValue: Sized {
    fn read(reader: &mut Reader<'_>) -> Step<Self>;
    fn write(&self, out: &mut Vec<u8>);
}

impl WireValue for PropValue {
    fn read(reader: &mut Reader<'_>) -> Step<Self> {
        reader.u32().map(Self)
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.put_u32(self.0);
    }
}

impl WireValue for PropRange {
    fn read(reader: &mut Reader<'_>) -> Step<Self> {
        let min = PropValue(reader.u32()?);
        let max = PropValue(reader.u32()?);
        Ok(Self { min, max })
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.put_u32(self.min.0);
        out.put_u32(self.max.0);
    }
}

/// Reads a bundle: a `u8` count, `count` `u8` IDs and `count` values.
pub(super) fn read_bundle<V: WireValue>(reader: &mut Reader<'_>) -> Step<PropBundle<V>> {
    let count = reader.u8()?;
    let ids = reader.take(usize::from(count))?;
    ids.iter()
        .map(|&id| {
            V::read(reader).map(|value| Prop {
                id: PropertyId(id),
                value,
            })
        })
        .collect()
}

/// Writes the bundle `bundle`.
///
/// # Errors
///
/// - [`Error::Io`](crate::Error::Io) when the bundle holds more than 255 properties.
pub(super) fn write_bundle<V: WireValue>(bundle: &PropBundle<V>, out: &mut Vec<u8>) -> Result<()> {
    out.put_u8(to_u8(bundle.props.len(), "property count")?);
    for prop in &bundle.props {
        out.put_u8(prop.id.0);
    }
    for prop in &bundle.props {
        prop.value.write(out);
    }
    Ok(())
}
