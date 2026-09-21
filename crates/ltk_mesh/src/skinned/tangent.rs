use std::collections::HashMap;

use bevy_mikktspace::{Geometry, TangentSpace};
use glam::{Vec2, Vec3, Vec4};

use crate::{
    error::BakeTangentsError,
    mem::{vertex::ElementName, VertexBuffer, VertexBufferDescription},
    SkinnedMesh, SkinnedMeshFlags, SkinnedMeshVertexType, MAX_VERTEX_COUNT,
};

type Result<T> = std::result::Result<T, BakeTangentsError>;

fn invalid(message: impl Into<String>) -> BakeTangentsError {
    BakeTangentsError::InvalidGeometry(message.into())
}

impl SkinnedMesh {
    /// Bakes MikkTSpace tangents from positions, normals and `Texcoord0` for League.
    ///
    /// Available with the `tangent-baking` feature.
    ///
    /// Writes unit, orthogonal `Vec4` tangents to `Texcoord6`. The stored `w` is
    /// the **negative** of MikkTSpace's sign for the stored UVs (equivalent to
    /// reversing V for tangent generation); the shader uses `B = w * cross(N, T)`.
    /// UVs and normals themselves are unchanged. Normal-map baking must use the
    /// same tangent basis and triangulation. This does not bake a texture.
    ///
    /// Basic and Color vertices become Tangent vertices, with opaque white added
    /// when colour is absent. Ext retains its extra UVs. Existing tangents are
    /// replaced. All other vertex bytes, bounds, flags, blend block and tail survive.
    ///
    /// Ranges are processed independently and repacked in range order. Vertices
    /// are split when face corners require different tangents; no input vertices
    /// are merged. Vertices outside all ranges are retained at the end. Indices
    /// remain range-relative, and `NORMALIZED_INDICES` is enabled if the resulting
    /// mesh exceeds 65536 vertices. External vertex references must be rebuilt.
    /// Degenerate or unused vertices receive a deterministic perpendicular tangent
    /// when MikkTSpace cannot supply one.
    ///
    /// # Errors
    /// Returns [`BakeTangentsError`] for unsupported layouts, non-finite attributes,
    /// zero normals, invalid or overlapping index ranges, out-of-range indices,
    /// non-triangle index counts, or overflow of the `.skn` limits after splitting.
    /// On error, the mesh is unchanged.
    ///
    /// ```no_run
    /// # fn example(mut mesh: ltk_mesh::SkinnedMesh) -> Result<(), Box<dyn std::error::Error>> {
    /// mesh.bake_tangents()?;
    /// mesh.to_writer(&mut std::fs::File::create("baked.skn")?)?;
    /// # Ok(()) }
    /// ```
    pub fn bake_tangents(&mut self) -> Result<()> {
        let source_type = self
            .vertex_type()
            .ok_or(BakeTangentsError::UnsupportedLayout)?;
        let output_type = if source_type == SkinnedMeshVertexType::Ext {
            SkinnedMeshVertexType::Ext
        } else {
            SkinnedMeshVertexType::Tangent
        };
        let vertices = &self.vertex_buffer;
        let positions: Vec<_> = vertices
            .accessor::<Vec3>(ElementName::Position)
            .ok_or(BakeTangentsError::UnsupportedLayout)?
            .iter()
            .collect();
        let normals: Vec<_> = vertices
            .accessor::<Vec3>(ElementName::Normal)
            .ok_or(BakeTangentsError::UnsupportedLayout)?
            .iter()
            .map(|n| n.try_normalize())
            .collect();
        let uvs: Vec<_> = vertices
            .accessor::<Vec2>(ElementName::Texcoord0)
            .ok_or(BakeTangentsError::UnsupportedLayout)?
            .iter()
            .collect();
        let mut unit_normals = Vec::with_capacity(normals.len());
        for (i, normal) in normals.into_iter().enumerate() {
            if !positions[i].is_finite() || !uvs[i].is_finite() || normal.is_none() {
                return Err(invalid(format!(
                    "vertex {i} has invalid position, normal or UV"
                )));
            }
            unit_normals.push(normal.ok_or_else(|| invalid("invalid normal"))?);
        }

        // Validate before passing any unchecked indices to the generator.
        let mut index_owners = vec![false; self.index_buffer.count()];
        let mut spans = Vec::with_capacity(self.ranges.len());
        for (r, range) in self.ranges.iter().enumerate() {
            let vs = checked_span(range.start_vertex, range.vertex_count, vertices.count())?;
            let is = checked_span(
                range.start_index,
                range.index_count,
                self.index_buffer.count(),
            )?;
            if !is.len().is_multiple_of(3) {
                return Err(invalid(format!("range {r} is not a triangle list")));
            }
            if vs.len() > MAX_VERTEX_COUNT as usize {
                return Err(BakeTangentsError::TooManyVertices(r));
            }
            for i in is.clone() {
                if std::mem::replace(&mut index_owners[i], true) {
                    return Err(invalid(format!("range {r} overlaps another index range")));
                }
                if self.index_buffer.get(i) as usize >= vs.len() {
                    return Err(invalid(format!("index {i} is outside range {r}")));
                }
            }
            spans.push((vs, is));
        }

        let stride = output_type.vertex_size();
        let mut bytes = Vec::new();
        let mut indices = self.index_buffer.clone();
        let mut ranges = self.ranges.clone();
        let mut used = vec![false; vertices.count()];
        for (r, (vs, is)) in spans.into_iter().enumerate() {
            let corners: Vec<_> = is
                .clone()
                .map(|i| vs.start + self.index_buffer.get(i) as usize)
                .collect();
            let mut geometry = TangentGeometry {
                positions: &positions,
                normals: &unit_normals,
                uvs: &uvs,
                tangents: vec![None; corners.len()],
                corners,
            };
            if !geometry.corners.is_empty() {
                bevy_mikktspace::generate_tangents(&mut geometry)
                    .map_err(|e| BakeTangentsError::Generation(e.to_string()))?;
            }
            let base = bytes.len() / stride;
            ranges[r].start_vertex = as_i32(base)?;
            for v in vs.clone() {
                used[v] = true;
                append_vertex(
                    &mut bytes,
                    vertices,
                    v,
                    source_type,
                    fallback(unit_normals[v]),
                );
            }
            let mut assigned = vec![false; vs.len()];
            let mut variants = HashMap::new();
            let mut count = vs.len();
            for (corner, index_slot) in is.enumerate() {
                let v = geometry.corners[corner];
                let local = v - vs.start;
                let tangent =
                    geometry.tangents[corner].unwrap_or_else(|| fallback(unit_normals[v]));
                // Canonicalize signed zeros for stable exact deduplication.
                let key = (
                    local,
                    tangent
                        .to_array()
                        .map(|f| if f == 0.0 { 0 } else { f.to_bits() }),
                );
                let output = if let Some(&output) = variants.get(&key) {
                    output
                } else {
                    let output = if !assigned[local] {
                        assigned[local] = true;
                        let offset = (base + local + 1) * stride - 16;
                        write_tangent(&mut bytes[offset..offset + 16], tangent);
                        local
                    } else {
                        if count == MAX_VERTEX_COUNT as usize {
                            return Err(BakeTangentsError::TooManyVertices(r));
                        }
                        append_vertex(&mut bytes, vertices, v, source_type, tangent);
                        count += 1;
                        count - 1
                    };
                    variants.insert(key, output);
                    output
                };
                indices.set(index_slot, output as u16);
            }
            ranges[r].vertex_count = as_i32(count)?;
        }
        for (v, used) in used.into_iter().enumerate() {
            if !used {
                append_vertex(
                    &mut bytes,
                    vertices,
                    v,
                    source_type,
                    fallback(unit_normals[v]),
                );
            }
        }
        let count = bytes.len() / stride;
        as_i32(count)?;
        // Commit only once all generation, splitting and validation have succeeded.
        self.vertex_buffer = VertexBuffer::new(VertexBufferDescription::from(output_type), bytes);
        self.index_buffer = indices;
        self.ranges = ranges;
        if count > MAX_VERTEX_COUNT as usize {
            self.flags.insert(SkinnedMeshFlags::NORMALIZED_INDICES);
        }
        Ok(())
    }
}

fn checked_span(start: i32, count: i32, limit: usize) -> Result<std::ops::Range<usize>> {
    let start = usize::try_from(start).map_err(|_| invalid("negative range start"))?;
    let count = usize::try_from(count).map_err(|_| invalid("negative range count"))?;
    let end = start
        .checked_add(count)
        .filter(|&end| end <= limit)
        .ok_or_else(|| invalid("range exceeds buffer"))?;
    Ok(start..end)
}

fn as_i32(value: usize) -> Result<i32> {
    i32::try_from(value).map_err(|_| invalid("vertex count exceeds i32::MAX"))
}

fn fallback(normal: Vec3) -> Vec4 {
    normal.any_orthonormal_vector().extend(-1.0)
}

fn append_vertex(
    bytes: &mut Vec<u8>,
    vertices: &VertexBuffer,
    v: usize,
    kind: SkinnedMeshVertexType,
    tangent: Vec4,
) {
    let source = &vertices.as_bytes()[v * vertices.stride()..(v + 1) * vertices.stride()];
    match kind {
        SkinnedMeshVertexType::Basic => {
            bytes.extend_from_slice(source);
            bytes.extend_from_slice(&[255; 4]);
        }
        SkinnedMeshVertexType::Color => bytes.extend_from_slice(source),
        SkinnedMeshVertexType::Tangent | SkinnedMeshVertexType::Ext => {
            bytes.extend_from_slice(&source[..source.len() - 16]);
        }
    }
    for f in tangent.to_array() {
        bytes.extend_from_slice(&f.to_le_bytes());
    }
}

fn write_tangent(bytes: &mut [u8], tangent: Vec4) {
    for (dest, f) in bytes.chunks_exact_mut(4).zip(tangent.to_array()) {
        dest.copy_from_slice(&f.to_le_bytes());
    }
}

struct TangentGeometry<'a> {
    positions: &'a [Vec3],
    normals: &'a [Vec3],
    uvs: &'a [Vec2],
    corners: Vec<usize>,
    tangents: Vec<Option<Vec4>>,
}

impl Geometry for TangentGeometry<'_> {
    fn num_faces(&self) -> usize {
        self.corners.len() / 3
    }
    fn num_vertices_of_face(&self, _: usize) -> usize {
        3
    }
    fn position(&self, face: usize, vert: usize) -> [f32; 3] {
        self.positions[self.corners[face * 3 + vert]].to_array()
    }
    fn normal(&self, face: usize, vert: usize) -> [f32; 3] {
        self.normals[self.corners[face * 3 + vert]].to_array()
    }
    fn tex_coord(&self, face: usize, vert: usize) -> [f32; 2] {
        self.uvs[self.corners[face * 3 + vert]].to_array()
    }
    fn set_tangent(&mut self, space: Option<TangentSpace>, face: usize, vert: usize) {
        let corner = face * 3 + vert;
        let normal = self.normals[self.corners[corner]];
        self.tangents[corner] = space.and_then(|space| {
            let t = Vec4::from_array(space.tangent_encoded());
            // Also handle isolated UV degeneracies and rounding against non-unit input normals.
            (t.truncate() - normal * normal.dot(t.truncate()))
                .try_normalize()
                .map(|xyz| xyz.extend(-t.w))
        });
    }
}
