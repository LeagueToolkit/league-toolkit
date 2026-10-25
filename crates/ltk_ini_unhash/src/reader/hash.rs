//! The SDBM hash of a name, built forwards from the state of a section.

use ltk_hash::{sdbm, IniHash};

use super::part::{each_byte, Parts};

/// The SDBM hash of a name from the state 0, and [`sdbm::MULTIPLIER`] to the power of the length
/// of the name.
///
/// SDBM is linear in its state. The hash of the name from the state `s` is `s * power + hash`.
#[derive(Debug, Clone, Copy)]
pub(super) struct NameHash {
    hash: u32,
    power: u32,
}

impl NameHash {
    pub(super) fn of(parts: Parts<'_>) -> Self {
        let (mut hash, mut power) = (0u32, 1u32);
        each_byte(parts, &mut |byte| {
            hash = sdbm::hash_lower_byte(hash, byte);
            power = power.wrapping_mul(sdbm::MULTIPLIER);
        });
        Self { hash, power }
    }

    /// Returns the SDBM state after the name, starting from the state `state`.
    pub(super) fn after(self, state: u32) -> u32 {
        state.wrapping_mul(self.power).wrapping_add(self.hash)
    }

    /// Returns the key of the name in a section with the state `state`.
    pub(super) fn key(self, state: u32) -> IniHash {
        IniHash(self.after(state))
    }
}

/// Returns the SDBM state after `text`, starting from the state `state`.
pub(super) fn extend(state: u32, text: &str) -> u32 {
    sdbm::hash_lower_from(state, text)
}

#[cfg(test)]
mod tests {
    use super::super::part::Part;
    use super::*;

    #[test]
    fn name_hash_continues_section_state() {
        let name = NameHash::of(&[Part::Str("GroupPart"), Part::Int(1)]);
        let state = extend(ltk_hash::sdbm::hash_lower("System"), "*");
        assert_eq!(name.key(state), IniHash(0x0616933a));
    }
}
