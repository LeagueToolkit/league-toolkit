use crate::{
    asset::{
        self,
        compressed::{
            evaluate::{JumpFrameU16, JumpFrameU32},
            frame::Frame,
        },
        error_metric::ErrorMetric,
    },
    AssetParseError::{InvalidField, InvalidFileVersion, MissingData},
    Compressed,
};
use bitflags::bitflags;
use std::io::{Read, Seek, SeekFrom};
use std::mem::size_of;

bitflags! {
    #[derive(Clone, Debug)]
    pub struct AnimationFlags: u32 {
        const Unk1 = 1 << 0;
        const Unk2 = 1 << 1;
        const UseKeyframeParametrization = 1 << 2;
    }
}

impl Compressed {
    /// Only use this if you already know the animation asset is compressed! If you aren't sure, please use AnimationAsset::from_reader
    ///
    /// # Errors
    ///
    /// Returns [`InvalidField`] for a negative jump cache count, a duration that is negative
    /// or not finite, and a frame whose transform type is `3` or whose joint id is not below
    /// the joint count. Returns [`ReaderError`](crate::AssetParseError::ReaderError) when the
    /// stream ends before the joints, frames or jump caches its header counts.
    pub fn from_reader<R: Read + Seek + ?Sized>(reader: &mut R) -> asset::Result<Self> {
        use byteorder::{ReadBytesExt as _, LE};
        use ltk_io_ext::{
            untrusted::{self, UntrustedCapacity as _},
            ReaderExt as _,
        };

        let _magic = reader.read_u64::<LE>()?; // magic is an 8 byte string

        let version = reader.read_u32::<LE>()?;
        if version != 1 && version != 2 && version != 3 {
            return Err(InvalidFileVersion(version));
        }

        let _resource_size = reader.read_u32::<LE>()?;
        let _format_token = reader.read_u32::<LE>()?;
        let flags = reader.read_u32::<LE>()?;
        let flags = AnimationFlags::from_bits(flags)
            .ok_or_else(|| InvalidField("flags", flags.to_string()))?;

        let joint_count = reader.read_u32::<LE>()?;
        let frame_count = reader.read_u32::<LE>()?;
        let jump_cache_count = reader.read_i32::<LE>()?;
        let jump_cache_count = u32::try_from(jump_cache_count)
            .map_err(|_| InvalidField("jump cache count", jump_cache_count.to_string()))?;

        let duration = reader.read_f32::<LE>()?;
        if !duration.is_finite() || duration < 0.0 {
            return Err(InvalidField("duration", duration.to_string()));
        }
        let fps = reader.read_f32::<LE>()?;

        let rotation_error_metric = ErrorMetric::from_reader(reader)?;
        let translation_error_metric = ErrorMetric::from_reader(reader)?;
        let scale_error_metric = ErrorMetric::from_reader(reader)?;

        let translation_min = reader.read_vec3::<LE>()?;
        let translation_max = reader.read_vec3::<LE>()?;

        let scale_min = reader.read_vec3::<LE>()?;
        let scale_max = reader.read_vec3::<LE>()?;

        let frames_off = reader.read_i32::<LE>()?;
        if frames_off <= 0 {
            return Err(MissingData("frame"));
        }
        let jump_caches_off = reader.read_i32::<LE>()?;
        if jump_caches_off <= 0 {
            return Err(MissingData("jump cache"));
        }
        let joint_name_hashes_off = reader.read_i32::<LE>()?;
        if joint_name_hashes_off <= 0 {
            return Err(MissingData("joint"));
        }

        // Read joint hashes
        reader.seek(SeekFrom::Start(joint_name_hashes_off as u64 + 12))?;
        let mut joints = Vec::with_untrusted_capacity(joint_count as usize);
        // TODO (alan): consider direct memory reinterp
        for _ in 0..joint_count {
            joints.push(reader.read_u32::<LE>()?);
        }

        // Read frames
        reader.seek(SeekFrom::Start(frames_off as u64 + 12))?;
        let mut frames = Vec::with_untrusted_capacity(frame_count as usize);
        for _ in 0..frame_count {
            let mut bytes = [0; Frame::SIZE];
            reader.read_exact(&mut bytes)?;
            let frame = Frame::from_bytes(bytes)?;
            if usize::from(frame.joint_id()) >= joints.len() {
                return Err(InvalidField("frame joint id", frame.joint_id().to_string()));
            }
            frames.push(frame);
        }

        // Read jump caches: one jump frame per joint per cache
        reader.seek(SeekFrom::Start(jump_caches_off as u64 + 12))?;
        let jump_frame_size = if frame_count < 0x10001 {
            size_of::<JumpFrameU16>()
        } else {
            size_of::<JumpFrameU32>()
        };
        let jump_caches_len = (jump_cache_count as usize)
            .checked_mul(joint_count as usize)
            .and_then(|jump_frames| jump_frames.checked_mul(jump_frame_size))
            .ok_or_else(|| InvalidField("jump cache count", jump_cache_count.to_string()))?;
        let jump_caches = untrusted::read_bytes(reader, jump_caches_len)?;

        Ok(Self {
            flags,
            duration,
            fps,
            rotation_error_metric,
            translation_error_metric,
            scale_error_metric,
            translation_min,
            translation_max,
            scale_min,
            scale_max,
            jump_cache_count: jump_cache_count as usize,
            frames,
            jump_caches,
            joints,
        })
    }
}
