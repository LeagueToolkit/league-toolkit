use std::fmt;

use ltk_hash::fnv1;

/// A Wwise object ID: the first `u32` of a hierarchy object.
///
/// The ID of an event, a switch, a state or a game parameter is the FNV-1 hash of its name.
///
/// ```
/// use ltk_wwise::ObjectId;
///
/// assert_eq!(ObjectId::from_name("Play_vo_Ahri_Attack"), ObjectId(0x2890d955));
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(pub u32);

impl ObjectId {
    /// Returns the ID of the name `name`: [`fnv1::hash_lower`].
    pub fn from_name(name: &str) -> Self {
        Self(fnv1::hash_lower(name))
    }
}

/// The ID of one media, in a media index or in a package entry name.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceId(pub u32);

/// The ID under which the client registers a bank.
///
/// ```
/// use ltk_wwise::BankId;
///
/// assert_eq!(BankId::from_name("sett_base_sfx_audio.bnk"), BankId(0xe9b70b40));
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BankId(pub u32);

impl BankId {
    /// Returns the ID of the bank file `name`: [`fnv1::bank_id`].
    pub fn from_name(name: &str) -> Self {
        Self(fnv1::bank_id(name))
    }
}

macro_rules! id_impls {
    ($($ty:ident),*) => {$(
        impl From<u32> for $ty {
            fn from(value: u32) -> Self {
                Self(value)
            }
        }

        impl From<$ty> for u32 {
            fn from(value: $ty) -> Self {
                value.0
            }
        }

        /// Writes the decimal value.
        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }
    )*};
}

id_impls!(ObjectId, SourceId, BankId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_display_in_decimal() {
        assert_eq!(SourceId(1004362275).to_string(), "1004362275");
        assert_eq!(ObjectId(0x10).to_string(), "16");
    }
}
