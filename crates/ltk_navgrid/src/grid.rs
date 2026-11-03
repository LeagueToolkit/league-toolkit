//! The `.aimesh_ngrid` navigation grid.

use std::io::{Read, Write};

use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use glam::Vec3;
use ltk_io_ext::{untrusted::UntrustedCapacity, ReaderExt, WriterExt};

use crate::{CellFlags, NavGridOverlay, ParseError, RegionOverlay, Result};

/// The grid version that [`NavGrid::to_writer`] writes.
pub const WRITE_VERSION: u8 = 7;

/// The number of region tag groups in a grid.
pub const REGION_TAG_GROUP_COUNT: usize = 8;

/// The number of tags in one region tag group.
pub const REGION_TAGS_PER_GROUP: usize = 16;

/// The size in bytes of the hint grid: 900 nodes of 900 `f32` distances and one 4-byte cell
/// locator each.
pub const HINT_GRID_SIZE: usize = 900 * (900 * 4 + 4);

/// One cell of a [`NavGrid`].
///
/// The game reads `height`, `flags` and `region_tags`. The other fields are a serialized
/// pathfinding record. The game skips the record.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NavGridCell {
    /// The world height of the cell center.
    pub height: f32,
    /// The pathfinding session id. Shipped grids store `0xFFFFFFFF`.
    pub session_id: u32,
    /// The pathfinding arrival cost.
    pub arrival_cost: f32,
    /// The pathfinding heuristic.
    pub heuristic: f32,
    /// A 32-bit actor list slot. Shipped grids store 0.
    pub actor_list: u32,
    /// The `(x, y)` coordinates of the cell. Shipped grids store the coordinates of the cell
    /// itself.
    pub locator: [i16; 2],
    /// The additional pathfinding cost.
    pub additional_cost: f32,
    /// The hint value of the cell as a goal cell.
    pub hint_as_good_cell: f32,
    /// The session id of the goal cell search.
    pub good_cell_session_id: i32,
    /// The reference hint weight.
    pub ref_hint_weight: f32,
    /// The reference count of the additional cost.
    pub additional_cost_ref_count: u16,
    /// The arrival direction.
    pub arrival_direction: u8,
    /// The open list state. Shipped grids store 0.
    pub is_open: u8,
    /// The two reference hint nodes. `0xFFFF` is no node.
    pub ref_hint_nodes: [u16; 2],
    /// The terrain flags.
    pub flags: CellFlags,
    /// Eight 4-bit region tag indices. Group `g` uses bits `4g` to `4g + 3`.
    pub region_tags: u32,
}

impl NavGridCell {
    /// Returns the index of the tag of region tag group `group` in this cell.
    ///
    /// The index selects one of the [`REGION_TAGS_PER_GROUP`] tags of the group. Index 0 is a
    /// tag, not the absence of a tag. Returns `None` if `group` is
    /// [`REGION_TAG_GROUP_COUNT`] or more.
    #[must_use]
    pub fn region_tag_index(&self, group: usize) -> Option<u8> {
        (group < REGION_TAG_GROUP_COUNT).then(|| ((self.region_tags >> (4 * group)) & 0xF) as u8)
    }
}

/// One tag slot of a [`RegionTagGroup`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct RegionTag {
    /// The FNV-1 hash of the lowercased tag name, the value of `ltk_hash::fnv1::hash_lower`.
    /// The hash is FNV-1, not the FNV-1a of bin names.
    pub name: u32,
    /// A second hash word. Shipped grids store 0.
    pub name2: u32,
}

/// One group of region tags, for example the music areas of a map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct RegionTagGroup {
    /// The FNV-1 hash of the lowercased group name, the value of `ltk_hash::fnv1::hash_lower`.
    /// 0 marks an unused group.
    pub name: u32,
    /// The tags of the group, indexed by the 4-bit value in each cell.
    pub tags: [RegionTag; REGION_TAGS_PER_GROUP],
}

/// A navigation grid: a `.aimesh_ngrid` file.
///
/// The grid covers the ground plane in square cells. Cell `(0, 0)` starts at
/// [`NavGrid::min_position`]. The cells span `x_cell_count * cell_size` world units along world
/// `x` and `y_cell_count * cell_size` world units along world `z`. The last column and the last
/// row can extend past [`NavGrid::max_position`]. World `x` maps to cell `x` and world `z` maps
/// to cell `y`.
///
/// A second, finer grid of sampled heights spans from `min_position` to `max_position`. The `y`
/// of `min_position` and `max_position` does not bound the sampled heights.
#[derive(Debug, Clone, PartialEq)]
pub struct NavGrid {
    flags: u16,
    min_position: Vec3,
    max_position: Vec3,
    cell_size: f32,
    x_cell_count: u32,
    y_cell_count: u32,
    cells: Vec<NavGridCell>,
    region_tag_groups: [RegionTagGroup; REGION_TAG_GROUP_COUNT],
    x_sample_count: u32,
    y_sample_count: u32,
    x_sample_spacing: f32,
    y_sample_spacing: f32,
    sampled_heights: Vec<f32>,
    hint_grid: Box<[u8]>,
}

/// Returns `width * height` as `usize`.
///
/// Fails with [`ParseError::CountOverflow`] if the product does not fit.
pub(crate) fn element_count(width: u32, height: u32) -> Result<usize> {
    usize::try_from(u64::from(width) * u64::from(height))
        .map_err(|_| ParseError::CountOverflow { width, height })
}

impl NavGrid {
    /// Reads a grid of version 6 or 7.
    ///
    /// A version 6 grid stores 16-bit region tags and 4 region tag groups. The reader widens the
    /// tags to 32 bits and leaves the other 4 groups empty.
    ///
    /// # Errors
    ///
    /// Fails with [`ParseError::UnsupportedVersion`] for any other version.
    /// Fails with [`ParseError::CountOverflow`] if a cell or sample count does not fit in memory.
    /// Fails with [`ParseError::Io`] if the stream ends early or a read fails.
    pub fn from_reader<R: Read + ?Sized>(reader: &mut R) -> Result<Self> {
        let version = reader.read_u8()?;
        if !(6..=7).contains(&version) {
            return Err(ParseError::UnsupportedVersion(version));
        }
        let wide_tags = version >= 7;

        let flags = reader.read_u16::<LE>()?;
        let min_position = reader.read_vec3::<LE>()?;
        let max_position = reader.read_vec3::<LE>()?;
        let cell_size = reader.read_f32::<LE>()?;
        let x_cell_count = reader.read_u32::<LE>()?;
        let y_cell_count = reader.read_u32::<LE>()?;
        let cell_count = element_count(x_cell_count, y_cell_count)?;

        let mut cells = Vec::with_untrusted_capacity(cell_count);
        for _ in 0..cell_count {
            cells.push(NavGridCell {
                height: reader.read_f32::<LE>()?,
                session_id: reader.read_u32::<LE>()?,
                arrival_cost: reader.read_f32::<LE>()?,
                heuristic: reader.read_f32::<LE>()?,
                actor_list: reader.read_u32::<LE>()?,
                locator: [reader.read_i16::<LE>()?, reader.read_i16::<LE>()?],
                additional_cost: reader.read_f32::<LE>()?,
                hint_as_good_cell: reader.read_f32::<LE>()?,
                good_cell_session_id: reader.read_i32::<LE>()?,
                ref_hint_weight: reader.read_f32::<LE>()?,
                additional_cost_ref_count: reader.read_u16::<LE>()?,
                arrival_direction: reader.read_u8()?,
                is_open: reader.read_u8()?,
                ref_hint_nodes: [reader.read_u16::<LE>()?, reader.read_u16::<LE>()?],
                flags: CellFlags::empty(),
                region_tags: 0,
            });
        }
        for cell in &mut cells {
            cell.flags = CellFlags::from_bits_retain(reader.read_u16::<LE>()?);
        }
        for cell in &mut cells {
            cell.region_tags = if wide_tags {
                reader.read_u32::<LE>()?
            } else {
                u32::from(reader.read_u16::<LE>()?)
            };
        }

        let mut region_tag_groups = [RegionTagGroup::default(); REGION_TAG_GROUP_COUNT];
        let group_count = if wide_tags { REGION_TAG_GROUP_COUNT } else { 4 };
        for group in &mut region_tag_groups[..group_count] {
            group.name = reader.read_u32::<LE>()?;
            for tag in &mut group.tags {
                tag.name = reader.read_u32::<LE>()?;
                tag.name2 = reader.read_u32::<LE>()?;
            }
        }

        let x_sample_count = reader.read_u32::<LE>()?;
        let y_sample_count = reader.read_u32::<LE>()?;
        let x_sample_spacing = reader.read_f32::<LE>()?;
        let y_sample_spacing = reader.read_f32::<LE>()?;
        let sample_count = element_count(x_sample_count, y_sample_count)?;
        let mut sampled_heights = Vec::with_untrusted_capacity(sample_count);
        for _ in 0..sample_count {
            sampled_heights.push(reader.read_f32::<LE>()?);
        }

        let mut hint_grid = vec![0; HINT_GRID_SIZE].into_boxed_slice();
        reader.read_exact(&mut hint_grid)?;

        Ok(Self {
            flags,
            min_position,
            max_position,
            cell_size,
            x_cell_count,
            y_cell_count,
            cells,
            region_tag_groups,
            x_sample_count,
            y_sample_count,
            x_sample_spacing,
            y_sample_spacing,
            sampled_heights,
            hint_grid,
        })
    }

    /// Writes the grid as version [`WRITE_VERSION`].
    ///
    /// # Errors
    ///
    /// Fails if a write to `writer` fails.
    pub fn to_writer<W: Write + ?Sized>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_u8(WRITE_VERSION)?;
        writer.write_u16::<LE>(self.flags)?;
        writer.write_vec3::<LE>(self.min_position)?;
        writer.write_vec3::<LE>(self.max_position)?;
        writer.write_f32::<LE>(self.cell_size)?;
        writer.write_u32::<LE>(self.x_cell_count)?;
        writer.write_u32::<LE>(self.y_cell_count)?;

        for cell in &self.cells {
            writer.write_f32::<LE>(cell.height)?;
            writer.write_u32::<LE>(cell.session_id)?;
            writer.write_f32::<LE>(cell.arrival_cost)?;
            writer.write_f32::<LE>(cell.heuristic)?;
            writer.write_u32::<LE>(cell.actor_list)?;
            writer.write_i16::<LE>(cell.locator[0])?;
            writer.write_i16::<LE>(cell.locator[1])?;
            writer.write_f32::<LE>(cell.additional_cost)?;
            writer.write_f32::<LE>(cell.hint_as_good_cell)?;
            writer.write_i32::<LE>(cell.good_cell_session_id)?;
            writer.write_f32::<LE>(cell.ref_hint_weight)?;
            writer.write_u16::<LE>(cell.additional_cost_ref_count)?;
            writer.write_u8(cell.arrival_direction)?;
            writer.write_u8(cell.is_open)?;
            writer.write_u16::<LE>(cell.ref_hint_nodes[0])?;
            writer.write_u16::<LE>(cell.ref_hint_nodes[1])?;
        }
        for cell in &self.cells {
            writer.write_u16::<LE>(cell.flags.bits())?;
        }
        for cell in &self.cells {
            writer.write_u32::<LE>(cell.region_tags)?;
        }

        for group in &self.region_tag_groups {
            writer.write_u32::<LE>(group.name)?;
            for tag in &group.tags {
                writer.write_u32::<LE>(tag.name)?;
                writer.write_u32::<LE>(tag.name2)?;
            }
        }

        writer.write_u32::<LE>(self.x_sample_count)?;
        writer.write_u32::<LE>(self.y_sample_count)?;
        writer.write_f32::<LE>(self.x_sample_spacing)?;
        writer.write_f32::<LE>(self.y_sample_spacing)?;
        for &height in &self.sampled_heights {
            writer.write_f32::<LE>(height)?;
        }

        writer.write_all(&self.hint_grid)
    }

    /// Returns the grid flags. Bit `0x2` marks a grid with at least one
    /// [`CellFlags::EXPOSED`] cell at save time. The game recomputes that bit on load.
    #[must_use]
    pub fn flags(&self) -> u16 {
        self.flags
    }

    /// Returns the minimum corner of the grid in world space.
    #[must_use]
    pub fn min_position(&self) -> Vec3 {
        self.min_position
    }

    /// Returns the maximum corner of the grid in world space.
    #[must_use]
    pub fn max_position(&self) -> Vec3 {
        self.max_position
    }

    /// Returns the edge length of one cell in world units.
    #[must_use]
    pub fn cell_size(&self) -> f32 {
        self.cell_size
    }

    /// Returns the number of cells along world `x`.
    #[must_use]
    pub fn x_cell_count(&self) -> u32 {
        self.x_cell_count
    }

    /// Returns the number of cells along world `z`.
    #[must_use]
    pub fn y_cell_count(&self) -> u32 {
        self.y_cell_count
    }

    /// Returns the cells in row-major order. Cell `(x, y)` has index
    /// `x + y * x_cell_count`.
    #[must_use]
    pub fn cells(&self) -> &[NavGridCell] {
        &self.cells
    }

    /// Returns the cells in row-major order as a mutable slice.
    #[must_use]
    pub fn cells_mut(&mut self) -> &mut [NavGridCell] {
        &mut self.cells
    }

    /// Returns the index of cell `(x, y)` in [`NavGrid::cells`]. Returns `None` if the cell is
    /// outside the grid.
    #[must_use]
    pub fn cell_index(&self, x: u32, y: u32) -> Option<usize> {
        (x < self.x_cell_count && y < self.y_cell_count)
            .then(|| x as usize + y as usize * self.x_cell_count as usize)
    }

    /// Returns the `(x, y)` coordinates of the cell at `index` in [`NavGrid::cells`]. Returns
    /// `None` if `index` is outside the cells.
    #[must_use]
    pub fn cell_coords(&self, index: usize) -> Option<(u32, u32)> {
        if index >= self.cells.len() {
            return None;
        }
        let width = self.x_cell_count as usize;
        Some((
            u32::try_from(index % width).ok()?,
            u32::try_from(index / width).ok()?,
        ))
    }

    /// Returns cell `(x, y)`. Returns `None` if the cell is outside the grid.
    #[must_use]
    pub fn cell(&self, x: u32, y: u32) -> Option<&NavGridCell> {
        self.cells.get(self.cell_index(x, y)?)
    }

    /// Returns cell `(x, y)` as a mutable reference. Returns `None` if the cell is outside the
    /// grid.
    #[must_use]
    pub fn cell_mut(&mut self, x: u32, y: u32) -> Option<&mut NavGridCell> {
        let index = self.cell_index(x, y)?;
        self.cells.get_mut(index)
    }

    /// Returns the world position of the center of cell `(x, y)`, at the cell height.
    /// Returns `None` if the cell is outside the grid.
    #[must_use]
    pub fn cell_center(&self, x: u32, y: u32) -> Option<Vec3> {
        let cell = self.cell(x, y)?;
        Some(Vec3::new(
            self.min_position.x + (x as f32 + 0.5) * self.cell_size,
            cell.height,
            self.min_position.z + (y as f32 + 0.5) * self.cell_size,
        ))
    }

    /// Returns the coordinates of the cell that contains the world `x` and `z` of `position`.
    /// The `y` of `position` has no effect. Returns `None` if the position is outside the grid.
    #[must_use]
    pub fn world_to_cell(&self, position: Vec3) -> Option<(u32, u32)> {
        let to_cell = |offset: f32, count: u32| {
            let cell = (offset / self.cell_size).floor();
            // `cell >= 0.0` is false for NaN. `as` saturates values above `u32::MAX`.
            (cell >= 0.0 && (cell as u32) < count).then_some(cell as u32)
        };
        Some((
            to_cell(position.x - self.min_position.x, self.x_cell_count)?,
            to_cell(position.z - self.min_position.z, self.y_cell_count)?,
        ))
    }

    /// Writes the flags of every rectangle of `overlay` into the cells that the rectangle covers.
    ///
    /// Each overlay value replaces the whole flag word of its cell. The rectangles apply in
    /// order. A later rectangle overwrites an earlier one where they overlap. The part of a
    /// rectangle outside the grid has no effect. A rectangle applies only its complete rows of
    /// `width` values, up to `height` rows.
    pub fn apply_overlay(&mut self, overlay: &NavGridOverlay) {
        for rect in &overlay.rects {
            self.apply_rect(
                (i64::from(rect.x), i64::from(rect.y)),
                (rect.width as usize, rect.height as usize),
                &rect.flags,
                |cell, flags| cell.flags = flags,
            );
        }
    }

    /// Writes the region tag words of every rectangle of `overlay` into the cells that the
    /// rectangle covers.
    ///
    /// Each overlay value replaces the whole region tag word of its cell, all
    /// [`REGION_TAG_GROUP_COUNT`] groups at once. The rectangles apply in order, with the same
    /// clipping as [`NavGrid::apply_overlay`].
    pub fn apply_region_overlay(&mut self, overlay: &RegionOverlay) {
        for rect in &overlay.rects {
            self.apply_rect(
                (i64::from(rect.x), i64::from(rect.y)),
                (usize::from(rect.width), usize::from(rect.height)),
                &rect.region_tags,
                |cell, region_tags| cell.region_tags = region_tags,
            );
        }
    }

    /// Calls `apply` for each value of a `width` x `height` rectangle with its first cell at
    /// `(x, y)`, where the value's cell is inside the grid.
    fn apply_rect<T: Copy>(
        &mut self,
        (x, y): (i64, i64),
        (width, height): (usize, usize),
        values: &[T],
        mut apply: impl FnMut(&mut NavGridCell, T),
    ) {
        if width == 0 {
            return;
        }
        for (row, cell_y) in values.chunks_exact(width).take(height).zip(y..) {
            let Ok(cell_y) = u32::try_from(cell_y) else {
                continue;
            };
            for (&value, cell_x) in row.iter().zip(x..) {
                if let Some(cell) = u32::try_from(cell_x)
                    .ok()
                    .and_then(|cell_x| self.cell_mut(cell_x, cell_y))
                {
                    apply(cell, value);
                }
            }
        }
    }

    /// Returns the region tag groups. A group with name 0 is unused.
    #[must_use]
    pub fn region_tag_groups(&self) -> &[RegionTagGroup; REGION_TAG_GROUP_COUNT] {
        &self.region_tag_groups
    }

    /// Returns the region tag groups as a mutable reference.
    #[must_use]
    pub fn region_tag_groups_mut(&mut self) -> &mut [RegionTagGroup; REGION_TAG_GROUP_COUNT] {
        &mut self.region_tag_groups
    }

    /// Returns the number of height samples along world `x`.
    #[must_use]
    pub fn x_sample_count(&self) -> u32 {
        self.x_sample_count
    }

    /// Returns the number of height samples along world `z`.
    #[must_use]
    pub fn y_sample_count(&self) -> u32 {
        self.y_sample_count
    }

    /// Returns the distance between two height samples along world `x`.
    #[must_use]
    pub fn x_sample_spacing(&self) -> f32 {
        self.x_sample_spacing
    }

    /// Returns the distance between two height samples along world `z`.
    #[must_use]
    pub fn y_sample_spacing(&self) -> f32 {
        self.y_sample_spacing
    }

    /// Returns the sampled heights in row-major order. Sample `(i, j)` has index
    /// `i + j * x_sample_count` and sits at world `x = min.x + i * x_sample_spacing`,
    /// `z = min.z + j * y_sample_spacing`.
    #[must_use]
    pub fn sampled_heights(&self) -> &[f32] {
        &self.sampled_heights
    }

    /// Returns the sampled heights in row-major order as a mutable slice.
    #[must_use]
    pub fn sampled_heights_mut(&mut self) -> &mut [f32] {
        &mut self.sampled_heights
    }

    /// Returns the height at the world `x` and `z` of `position`, interpolated between the four
    /// sampled heights around it. The `y` of `position` has no effect.
    ///
    /// Returns `None` if the position is outside the samples. The samples span from
    /// `min_position.x` to `min_position.x + (x_sample_count - 1) * x_sample_spacing` along world
    /// `x`. The span along world `z` uses the `y` sample count and spacing.
    #[must_use]
    pub fn sampled_height_at(&self, position: Vec3) -> Option<f32> {
        let (x0, x1, tx) = sample_span(
            position.x - self.min_position.x,
            self.x_sample_spacing,
            self.x_sample_count,
        )?;
        let (z0, z1, tz) = sample_span(
            position.z - self.min_position.z,
            self.y_sample_spacing,
            self.y_sample_count,
        )?;
        let width = self.x_sample_count as usize;
        let height = |x: usize, z: usize| self.sampled_heights.get(x + z * width).copied();
        let near = lerp(height(x0, z0)?, height(x1, z0)?, tx);
        let far = lerp(height(x0, z1)?, height(x1, z1)?, tx);
        Some(lerp(near, far, tz))
    }

    /// Returns the raw bytes of the hint grid, [`HINT_GRID_SIZE`] long. The game skips the hint
    /// grid.
    #[must_use]
    pub fn hint_grid(&self) -> &[u8] {
        &self.hint_grid
    }
}

/// Returns the indices of the two samples around `offset` along one axis, and the weight of the
/// second sample. Returns `None` if `offset` is outside the samples.
fn sample_span(offset: f32, spacing: f32, count: u32) -> Option<(usize, usize, f32)> {
    let last = count.checked_sub(1)?;
    let position = offset / spacing;
    // `contains` is false for NaN
    if !(0.0..=last as f32).contains(&position) {
        return None;
    }
    let first = (position.floor() as u32).min(last);
    let second = (first + 1).min(last);
    Some((first as usize, second as usize, position - first as f32))
}

fn lerp(from: f32, to: f32, weight: f32) -> f32 {
    from + (to - from) * weight
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NavGridOverlayRect, RegionOverlayRect};

    /// Returns a grid of `width` x `height` cells and 3 x 2 samples in `version`.
    fn grid_bytes(version: u8, width: u32, height: u32) -> Vec<u8> {
        let mut out = vec![version];
        out.extend(1u16.to_le_bytes());
        for v in [0.0f32, 0.0, 0.0, 100.0, 0.0, 50.0, 50.0] {
            out.extend(v.to_le_bytes());
        }
        out.extend(width.to_le_bytes());
        out.extend(height.to_le_bytes());
        let count = (width * height) as usize;
        for i in 0..count {
            out.extend(1.5f32.to_le_bytes());
            out.extend(u32::MAX.to_le_bytes());
            out.extend([0; 8]);
            out.extend(0u32.to_le_bytes());
            out.extend((i as i16 % width as i16).to_le_bytes());
            out.extend((i as i16 / width as i16).to_le_bytes());
            out.extend([0; 12]);
            out.extend(f32::NAN.to_le_bytes());
            out.extend(0u16.to_le_bytes());
            out.extend([9, 0]);
            out.extend([0xFF; 4]);
        }
        for i in 0..count {
            out.extend([0x0442u16, 0x8001][i % 2].to_le_bytes());
        }
        for i in 0..count {
            if version >= 7 {
                out.extend((0x8765_4321u32 + i as u32).to_le_bytes());
            } else {
                out.extend((0x4321u16 + i as u16).to_le_bytes());
            }
        }
        let groups = if version >= 7 { 8 } else { 4 };
        for g in 0..groups {
            out.extend((g as u32 + 1).to_le_bytes());
            for t in 0..16u32 {
                out.extend((g as u32 * 100 + t).to_le_bytes());
                out.extend(0u32.to_le_bytes());
            }
        }
        out.extend(3u32.to_le_bytes());
        out.extend(2u32.to_le_bytes());
        out.extend(50.0f32.to_le_bytes());
        out.extend(50.0f32.to_le_bytes());
        for i in 0..6 {
            out.extend((i as f32).to_le_bytes());
        }
        out.extend(vec![0xAB; HINT_GRID_SIZE]);
        out
    }

    #[test]
    fn nav_grid_from_reader_reads_version_7() {
        let grid = NavGrid::from_reader(&mut &grid_bytes(7, 2, 1)[..]).unwrap();
        assert_eq!(grid.flags(), 1);
        assert_eq!(grid.max_position(), Vec3::new(100.0, 0.0, 50.0));
        assert_eq!((grid.x_cell_count(), grid.y_cell_count()), (2, 1));
        let cell = grid.cell(1, 0).unwrap();
        assert_eq!(cell.height, 1.5);
        assert_eq!(cell.locator, [1, 0]);
        assert_eq!(cell.arrival_direction, 9);
        assert!(cell.ref_hint_weight.is_nan());
        assert_eq!(cell.flags.bits(), 0x8001);
        assert_eq!(cell.region_tags, 0x8765_4322);
        assert_eq!(cell.region_tag_index(0), Some(2));
        assert_eq!(cell.region_tag_index(7), Some(8));
        assert_eq!(cell.region_tag_index(8), None);
        assert_eq!(grid.region_tag_groups()[7].name, 8);
        assert_eq!(grid.region_tag_groups()[7].tags[15].name, 715);
        assert_eq!(grid.sampled_heights(), [0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
        assert!(grid.hint_grid().iter().all(|&b| b == 0xAB));
    }

    #[test]
    fn nav_grid_to_writer_round_trips_version_7() {
        let bytes = grid_bytes(7, 2, 3);
        let grid = NavGrid::from_reader(&mut &bytes[..]).unwrap();
        let mut written = Vec::new();
        grid.to_writer(&mut written).unwrap();
        assert_eq!(written, bytes);
    }

    #[test]
    fn nav_grid_from_reader_widens_version_6_region_tags() {
        let grid = NavGrid::from_reader(&mut &grid_bytes(6, 2, 1)[..]).unwrap();
        assert_eq!(grid.cell(1, 0).unwrap().region_tags, 0x4322);
        assert_eq!(grid.region_tag_groups()[3].name, 4);
        assert_eq!(grid.region_tag_groups()[4], RegionTagGroup::default());

        let mut written = Vec::new();
        grid.to_writer(&mut written).unwrap();
        let back = NavGrid::from_reader(&mut &written[..]).unwrap();
        assert_eq!(written[0], WRITE_VERSION);
        assert_eq!(back.cells()[1].region_tags, 0x4322);
        assert_eq!(back.region_tag_groups(), grid.region_tag_groups());
    }

    #[test]
    fn nav_grid_from_reader_fails_on_unsupported_version() {
        for version in [5, 8] {
            let error = NavGrid::from_reader(&mut &grid_bytes(version, 1, 1)[..]).unwrap_err();
            assert!(matches!(error, ParseError::UnsupportedVersion(v) if v == version));
        }
    }

    #[test]
    fn nav_grid_from_reader_fails_on_truncated_stream() {
        let bytes = grid_bytes(7, 2, 1);
        let error = NavGrid::from_reader(&mut &bytes[..bytes.len() - 1]).unwrap_err();
        assert!(
            matches!(error, ParseError::Io(e) if e.kind() == std::io::ErrorKind::UnexpectedEof)
        );
    }

    #[test]
    fn nav_grid_from_reader_fails_on_huge_count_without_allocating() {
        let mut bytes = grid_bytes(7, 1, 1);
        bytes[31..39].fill(0xFF);
        assert!(NavGrid::from_reader(&mut &bytes[..]).is_err());
    }

    #[test]
    fn nav_grid_world_to_cell_inverts_cell_center() {
        let grid = NavGrid::from_reader(&mut &grid_bytes(7, 2, 1)[..]).unwrap();
        let center = grid.cell_center(1, 0).unwrap();
        assert_eq!(center, Vec3::new(75.0, 1.5, 25.0));
        assert_eq!(grid.world_to_cell(center), Some((1, 0)));
        assert_eq!(grid.world_to_cell(Vec3::new(0.0, 9.0, 49.9)), Some((0, 0)));
        assert_eq!(grid.world_to_cell(Vec3::new(100.0, 0.0, 0.0)), None);
        assert_eq!(grid.world_to_cell(Vec3::new(-0.1, 0.0, 0.0)), None);
        assert_eq!(grid.world_to_cell(Vec3::new(f32::NAN, 0.0, 0.0)), None);
        assert_eq!(grid.cell_center(2, 0), None);
    }

    #[test]
    fn nav_grid_cell_coords_inverts_cell_index() {
        let grid = NavGrid::from_reader(&mut &grid_bytes(7, 2, 3)[..]).unwrap();
        assert_eq!(grid.cell_index(1, 2), Some(5));
        assert_eq!(grid.cell_coords(5), Some((1, 2)));
        assert_eq!(grid.cell_index(2, 0), None);
        assert_eq!(grid.cell_coords(6), None);
    }

    #[test]
    fn nav_grid_sampled_height_at_interpolates_samples() {
        // 3 x 2 samples 50 units apart, sample (i, j) holds height i + 3 * j
        let grid = NavGrid::from_reader(&mut &grid_bytes(7, 2, 1)[..]).unwrap();
        assert_eq!(grid.sampled_height_at(Vec3::new(0.0, 9.0, 0.0)), Some(0.0));
        assert_eq!(
            grid.sampled_height_at(Vec3::new(25.0, 0.0, 25.0)),
            Some(2.0)
        );
        assert_eq!(grid.sampled_height_at(Vec3::new(75.0, 0.0, 0.0)), Some(1.5));
        assert_eq!(
            grid.sampled_height_at(Vec3::new(100.0, 0.0, 50.0)),
            Some(5.0)
        );
        assert_eq!(grid.sampled_height_at(Vec3::new(100.1, 0.0, 0.0)), None);
        assert_eq!(grid.sampled_height_at(Vec3::new(0.0, 0.0, -0.1)), None);
        assert_eq!(grid.sampled_height_at(Vec3::new(f32::NAN, 0.0, 0.0)), None);
    }

    #[test]
    fn nav_grid_apply_overlay_replaces_flags_inside_grid() {
        let mut grid = NavGrid::from_reader(&mut &grid_bytes(7, 2, 1)[..]).unwrap();
        let overlay = NavGridOverlay {
            rects: vec![
                NavGridOverlayRect {
                    x: -1,
                    y: 0,
                    width: 2,
                    height: 1,
                    flags: vec![CellFlags::GRASS, CellFlags::WALL],
                },
                NavGridOverlayRect {
                    x: 1,
                    y: 0,
                    width: 2,
                    height: 2,
                    flags: vec![CellFlags::empty(); 4],
                },
            ],
            ..Default::default()
        };
        grid.apply_overlay(&overlay);
        assert_eq!(grid.cell(0, 0).unwrap().flags, CellFlags::WALL);
        assert_eq!(grid.cell(1, 0).unwrap().flags, CellFlags::empty());
    }

    #[test]
    fn nav_grid_apply_overlay_skips_incomplete_rows() {
        let mut grid = NavGrid::from_reader(&mut &grid_bytes(7, 2, 1)[..]).unwrap();
        let overlay = NavGridOverlay {
            rects: vec![
                NavGridOverlayRect {
                    width: 0,
                    height: 1,
                    flags: vec![CellFlags::WALL],
                    ..Default::default()
                },
                NavGridOverlayRect {
                    width: 2,
                    height: 1,
                    flags: vec![CellFlags::WALL],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        grid.apply_overlay(&overlay);
        assert_eq!(grid.cell(0, 0).unwrap().flags.bits(), 0x0442);
    }

    #[test]
    fn nav_grid_apply_region_overlay_replaces_region_tags() {
        let mut grid = NavGrid::from_reader(&mut &grid_bytes(7, 2, 1)[..]).unwrap();
        let overlay = RegionOverlay {
            version: 1,
            rects: vec![RegionOverlayRect {
                x: 1,
                y: 0,
                width: 1,
                height: 1,
                region_tags: vec![0xDEAD_BEEF],
            }],
        };
        grid.apply_region_overlay(&overlay);
        assert_eq!(grid.cell(0, 0).unwrap().region_tags, 0x8765_4321);
        assert_eq!(grid.cell(1, 0).unwrap().region_tags, 0xDEAD_BEEF);
    }
}
