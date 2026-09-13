//! Uncompressed animation reading (r3d2anmd)
//!
//! Supports versions:
//! - v5: Quantized quaternions (6 bytes), separate joint hash section
//! - v4: Full quaternions (16 bytes), joint hashes in frame data
//! - v3: Legacy format with 32-byte padded joint names

use crate::{
    asset::{self, uncompressed::UncompressedFrame},
    quantized, rotation, Uncompressed,
};
use byteorder::{ReadBytesExt, LE};
use glam::Vec3;
use ltk_hash::elf;
use ltk_io_ext::{untrusted::UntrustedCapacity, ReaderExt};
use std::collections::{hash_map::Entry, HashMap};
use std::io::{Read, Seek, SeekFrom};

/// Returns the element count of the section from `start` to `end`.
///
/// # Errors
///
/// Returns [`InvalidField`](asset::AssetParseError::InvalidField) when `end` is before
/// `start`, and when the size is not a multiple of `element_size`.
fn section_count(
    section_name: &'static str,
    start: i32,
    end: i32,
    element_size: usize,
) -> asset::Result<usize> {
    let size = usize::try_from(end - start).map_err(|_| {
        asset::AssetParseError::InvalidField(
            section_name,
            format!("ends at offset {end}, before its start at offset {start}"),
        )
    })?;
    if !size.is_multiple_of(element_size) {
        return Err(asset::AssetParseError::InvalidField(
            section_name,
            format!(
                "invalid size {}; expected multiple of {} bytes",
                size, element_size
            ),
        ));
    }
    Ok(size / element_size)
}

/// Returns the frame rate and the duration of a v4 or v5 clip.
///
/// # Errors
///
/// Returns [`InvalidField`](asset::AssetParseError::InvalidField) for a frame duration that
/// is not positive and finite, and for a frame rate or a clip duration that is not finite.
fn clip_timing(frame_count: usize, frame_duration: f32) -> asset::Result<(f32, f32)> {
    let fps = 1.0 / frame_duration;
    let duration = frame_count as f32 * frame_duration;
    if frame_duration > 0.0 && frame_duration.is_finite() && fps.is_finite() && duration.is_finite()
    {
        Ok((fps, duration))
    } else {
        Err(asset::AssetParseError::InvalidField(
            "frame duration",
            frame_duration.to_string(),
        ))
    }
}

impl Uncompressed {
    /// Parses an uncompressed animation from a reader
    ///
    /// Only use this if you already know the animation asset is uncompressed!
    /// If you aren't sure, please use `AnimationAsset::from_reader`
    ///
    /// # Errors
    ///
    /// Returns [`InvalidField`](asset::AssetParseError::InvalidField) for a v4 or v5 frame
    /// duration that is not positive and finite, or whose clip duration is not finite. Returns
    /// it for v4 or v5 section offsets out of order, for more joints than tracks, and for a
    /// rotation with no unit length. Returns [`ReaderError`](asset::AssetParseError::ReaderError)
    /// when the stream ends before the data its header counts.
    pub fn from_reader<R: Read + Seek + ?Sized>(reader: &mut R) -> asset::Result<Self> {
        let _magic = reader.read_u64::<LE>()?; // "r3d2anmd"
        let version = reader.read_u32::<LE>()?;

        match version {
            5 => Self::read_v5(reader),
            4 => Self::read_v4(reader),
            3 => Self::read_v3_legacy(reader),
            _ => Err(asset::AssetParseError::InvalidFileVersion(version)),
        }
    }

    /// Reads v5 format (newest)
    ///
    /// - Joint hashes in separate section
    /// - Quaternions quantized to 6 bytes
    fn read_v5<R: Read + Seek + ?Sized>(reader: &mut R) -> asset::Result<Self> {
        let _resource_size = reader.read_u32::<LE>()?;
        let _format_token = reader.read_u32::<LE>()?;
        let _version = reader.read_u32::<LE>()?;
        let _flags = reader.read_u32::<LE>()?;

        let track_count = reader.read_u32::<LE>()? as usize;
        let frame_count = reader.read_u32::<LE>()? as usize;
        let frame_duration = reader.read_f32::<LE>()?;
        let (fps, duration) = clip_timing(frame_count, frame_duration)?;

        let joint_hashes_offset = reader.read_i32::<LE>()?;
        let _asset_name_offset = reader.read_i32::<LE>()?;
        let _time_offset = reader.read_i32::<LE>()?;
        let vector_palette_offset = reader.read_i32::<LE>()?;
        let quat_palette_offset = reader.read_i32::<LE>()?;
        let frames_offset = reader.read_i32::<LE>()?;

        if joint_hashes_offset <= 0 {
            return Err(asset::AssetParseError::MissingData("joint hashes"));
        }
        if vector_palette_offset <= 0 {
            return Err(asset::AssetParseError::MissingData("vector palette"));
        }
        if quat_palette_offset <= 0 {
            return Err(asset::AssetParseError::MissingData("quaternion palette"));
        }
        if frames_offset <= 0 {
            return Err(asset::AssetParseError::MissingData("frames"));
        }

        // Sections in file order: vector palette, quaternion palette, joint hashes, frames
        let joint_hash_count =
            section_count("joint hashes", joint_hashes_offset, frames_offset, 4)?;
        let vector_count = section_count(
            "vector palette",
            vector_palette_offset,
            quat_palette_offset,
            12,
        )?;
        let quat_count = section_count(
            "quaternion palette",
            quat_palette_offset,
            joint_hashes_offset,
            6,
        )?;
        // Each joint hash names one track
        if joint_hash_count > track_count {
            return Err(asset::AssetParseError::InvalidField(
                "joint hashes",
                format!("{joint_hash_count} joint hashes for {track_count} tracks"),
            ));
        }

        // Read joint hashes
        reader.seek(SeekFrom::Start(joint_hashes_offset as u64 + 12))?;
        let mut joint_hashes = Vec::with_untrusted_capacity(joint_hash_count);
        for _ in 0..joint_hash_count {
            joint_hashes.push(reader.read_u32::<LE>()?);
        }

        // Read vector palette
        reader.seek(SeekFrom::Start(vector_palette_offset as u64 + 12))?;
        let mut vector_palette = Vec::with_untrusted_capacity(vector_count);
        for _ in 0..vector_count {
            vector_palette.push(reader.read_vec3::<LE>()?);
        }

        // Read quaternion palette (6-byte quantized)
        reader.seek(SeekFrom::Start(quat_palette_offset as u64 + 12))?;
        let mut quat_palette = Vec::with_untrusted_capacity(quat_count);
        for _ in 0..quat_count {
            let mut bytes = [0u8; 6];
            reader.read_exact(&mut bytes)?;
            quat_palette.push(quantized::decompress_quat(&bytes).normalize());
        }

        // Read frames. Track `i` holds the frames of joint hash `i`.
        let mut tracks = vec![Vec::new(); joint_hashes.len()];
        reader.seek(SeekFrom::Start(frames_offset as u64 + 12))?;
        for _ in 0..frame_count {
            for track_id in 0..track_count {
                let translation_id = reader.read_u16::<LE>()?;
                let scale_id = reader.read_u16::<LE>()?;
                let rotation_id = reader.read_u16::<LE>()?;

                // Skip tracks without a valid joint hash
                if let Some(frames) = tracks.get_mut(track_id) {
                    frames.push(UncompressedFrame {
                        translation_id,
                        scale_id,
                        rotation_id,
                    });
                }
            }
        }
        // The last track of a joint hash holds its frames.
        let joint_frames: HashMap<u32, Vec<UncompressedFrame>> =
            joint_hashes.into_iter().zip(tracks).collect();

        Ok(Self {
            duration,
            fps,
            frame_count,
            vector_palette,
            quat_palette,
            joint_frames,
        })
    }

    /// Reads v4 format
    ///
    /// - Joint hashes embedded in frame data
    /// - Full 16-byte quaternions
    fn read_v4<R: Read + Seek + ?Sized>(reader: &mut R) -> asset::Result<Self> {
        let _resource_size = reader.read_u32::<LE>()?;
        let _format_token = reader.read_u32::<LE>()?;
        let _version = reader.read_u32::<LE>()?;
        let _flags = reader.read_u32::<LE>()?;

        let track_count = reader.read_u32::<LE>()? as usize;
        let frame_count = reader.read_u32::<LE>()? as usize;
        let frame_duration = reader.read_f32::<LE>()?;
        let (fps, duration) = clip_timing(frame_count, frame_duration)?;

        let _joint_hashes_offset = reader.read_i32::<LE>()?;
        let _asset_name_offset = reader.read_i32::<LE>()?;
        let _time_offset = reader.read_i32::<LE>()?;
        let vector_palette_offset = reader.read_i32::<LE>()?;
        let quat_palette_offset = reader.read_i32::<LE>()?;
        let frames_offset = reader.read_i32::<LE>()?;

        if vector_palette_offset <= 0 {
            return Err(asset::AssetParseError::MissingData("vector palette"));
        }
        if quat_palette_offset <= 0 {
            return Err(asset::AssetParseError::MissingData("quaternion palette"));
        }
        if frames_offset <= 0 {
            return Err(asset::AssetParseError::MissingData("frames"));
        }

        // Sections in file order: vector palette, quaternion palette, frames
        let vector_count = section_count(
            "vector palette",
            vector_palette_offset,
            quat_palette_offset,
            12,
        )?;
        let quat_count = section_count(
            "quaternion palette",
            quat_palette_offset,
            frames_offset,
            16, // v4 uses full 16-byte quaternions
        )?;

        // Read vector palette
        reader.seek(SeekFrom::Start(vector_palette_offset as u64 + 12))?;
        let mut vector_palette = Vec::with_untrusted_capacity(vector_count);
        for _ in 0..vector_count {
            vector_palette.push(reader.read_vec3::<LE>()?);
        }

        // Read quaternion palette (full 16-byte)
        reader.seek(SeekFrom::Start(quat_palette_offset as u64 + 12))?;
        let mut quat_palette = Vec::with_untrusted_capacity(quat_count);
        for _ in 0..quat_count {
            let quat = reader.read_quat::<LE>()?;
            let quat = rotation::try_normalize(quat).ok_or_else(|| {
                asset::AssetParseError::InvalidField("quaternion palette", quat.to_string())
            })?;
            quat_palette.push(quat);
        }

        // Read frames - joint hash is embedded in each frame
        let mut joint_frames: HashMap<u32, Vec<UncompressedFrame>> =
            HashMap::with_untrusted_capacity(track_count);

        reader.seek(SeekFrom::Start(frames_offset as u64 + 12))?;
        for frame_id in 0..frame_count {
            for _ in 0..track_count {
                let joint_hash = reader.read_u32::<LE>()?;
                let translation_id = reader.read_u16::<LE>()?;
                let scale_id = reader.read_u16::<LE>()?;
                let rotation_id = reader.read_u16::<LE>()?;
                let _padding = reader.read_u16::<LE>()?;

                // Each joint takes one track.
                let joint_count = joint_frames.len();
                let frames = match joint_frames.entry(joint_hash) {
                    Entry::Occupied(entry) => entry.into_mut(),
                    Entry::Vacant(entry) if joint_count < track_count => entry.insert(Vec::new()),
                    Entry::Vacant(_) => {
                        return Err(asset::AssetParseError::InvalidField(
                            "joint hashes",
                            format!("more joint hashes than the {track_count} tracks"),
                        ));
                    }
                };
                // A joint holds the default frame at each frame that does not key it.
                if frames.len() <= frame_id {
                    frames.resize(frame_id + 1, UncompressedFrame::default());
                }
                frames[frame_id] = UncompressedFrame {
                    translation_id,
                    scale_id,
                    rotation_id,
                };
            }
        }
        for frames in joint_frames.values_mut() {
            frames.resize(frame_count, UncompressedFrame::default());
        }

        Ok(Self {
            duration,
            fps,
            frame_count,
            vector_palette,
            quat_palette,
            joint_frames,
        })
    }

    /// Reads v3 legacy format
    ///
    /// - 32-byte padded joint names (hashed using ELF)
    /// - Per-track frame storage
    /// - No scale support (defaults to 1.0)
    fn read_v3_legacy<R: Read + Seek + ?Sized>(reader: &mut R) -> asset::Result<Self> {
        let _skeleton_id = reader.read_u32::<LE>()?;
        let track_count = reader.read_u32::<LE>()? as usize;
        let frame_count = reader.read_u32::<LE>()? as usize;
        let fps = reader.read_u32::<LE>()? as f32;

        let duration = frame_count as f32 / fps;

        // Build palettes and frames as we read
        let key_count = frame_count.saturating_mul(track_count);
        let mut quat_palette = Vec::with_untrusted_capacity(key_count);
        let mut vector_palette = Vec::with_untrusted_capacity(key_count.saturating_add(1));
        let mut joint_frames: HashMap<u32, Vec<UncompressedFrame>> =
            HashMap::with_untrusted_capacity(track_count);

        // Add artificial static scale vector at index 0
        vector_palette.push(Vec3::ONE);

        for _ in 0..track_count {
            // Read 32-byte padded joint name and hash it
            let joint_name: String = reader.read_padded_string::<LE, 32>()?;
            let joint_hash = elf::elf(&joint_name) as u32;
            let _flags = reader.read_u32::<LE>()?;

            let mut frames = Vec::with_untrusted_capacity(frame_count);

            for _ in 0..frame_count {
                // Read rotation (quaternion) and translation directly
                let rotation = reader.read_quat::<LE>()?;
                let translation = reader.read_vec3::<LE>()?;

                let rotation_id = quat_palette.len() as u16;
                quat_palette.push(rotation);

                let translation_id = vector_palette.len() as u16;
                vector_palette.push(translation);

                // Scale is always 1.0 (index 0)
                frames.push(UncompressedFrame {
                    translation_id,
                    scale_id: 0,
                    rotation_id,
                });
            }

            joint_frames.insert(joint_hash, frames);
        }

        Ok(Self {
            duration,
            fps,
            frame_count,
            vector_palette,
            quat_palette,
            joint_frames,
        })
    }
}
