//! Per-cell terrain flags.

use bitflags::bitflags;

bitflags! {
    /// Terrain flags of one navigation grid cell.
    ///
    /// The named values are the terrain tags of the game's navigation grid configuration. Some
    /// tags set two bits. [`CellFlags::contains`] tests a whole tag. [`CellFlags::bits`] gives
    /// access to a single bit.
    ///
    /// A value keeps every bit, including bits that no tag names. Bits `0x8` and `0x80` belong
    /// to no tag. The Summoner's Rift grid sets `0x80` in about a third of its cells. A color
    /// scheme built from these constants alone leaves those cells without a tag.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct CellFlags: u16 {
        /// The `Grass` tag.
        const GRASS = 0x0001;
        /// The `Wall` tag. A cell without a team bit is passable if this bit is clear.
        const WALL = 0x0002;
        /// The `Transient` tag.
        const TRANSIENT = 0x0004;
        /// The `TransparentWall` tag.
        const TRANSPARENT_WALL = 0x0042;
        /// The `Exposed` tag.
        const EXPOSED = 0x0100;
        /// The `OpaqueWall` tag.
        const OPAQUE_WALL = 0x0202;
        /// The `OnlyPassableByTeamOrder` tag.
        const ONLY_PASSABLE_BY_TEAM_ORDER = 0x0440;
        /// The `OnlyPassableByTeamChaos` tag.
        const ONLY_PASSABLE_BY_TEAM_CHAOS = 0x0840;
        /// The `GrassOnlyPassableByTeamOrder` tag.
        const GRASS_ONLY_PASSABLE_BY_TEAM_ORDER = 0x0401;
        /// The `GrassOnlyPassableByTeamChaos` tag.
        const GRASS_ONLY_PASSABLE_BY_TEAM_CHAOS = 0x0801;
        /// The `OnlyPassableByTeamOrderNeutralZoneVisibility` tag.
        const ONLY_PASSABLE_BY_TEAM_ORDER_NEUTRAL_ZONE_VISIBILITY = 0x1440;
        /// The `OnlyPassableByTeamChaosNeutralZoneVisibility` tag.
        const ONLY_PASSABLE_BY_TEAM_CHAOS_NEUTRAL_ZONE_VISIBILITY = 0x1840;
        /// The `GrassWall` tag.
        const GRASS_WALL = 0x0003;

        const _ = !0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_flags_from_bits_retain_keeps_unnamed_bits() {
        let flags = CellFlags::from_bits_retain(0x8088);
        assert_eq!(flags.bits(), 0x8088);
    }

    #[test]
    fn cell_flags_contains_needs_every_bit_of_a_tag() {
        assert!(
            !CellFlags::from_bits_retain(0x0400).contains(CellFlags::ONLY_PASSABLE_BY_TEAM_ORDER)
        );
        assert!(
            CellFlags::from_bits_retain(0x0442).contains(CellFlags::ONLY_PASSABLE_BY_TEAM_ORDER)
        );
    }
}
