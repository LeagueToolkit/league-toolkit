use std::io;
use std::io::{Read, Seek, SeekFrom};
use byteorder::ReadBytesExt;
use glam::{Mat4, Quat, Vec3};
use crate::util::ReaderExt;

#[derive(Debug, Clone, PartialEq)]
pub struct Joint {
    name: String,
    flags: u16,
    id: i16,
    parent_id: i16,
    radius: f32,
    local_transform: Mat4,
    local_translation: Vec3,
    local_scale: Vec3,
    local_rotation: Quat,
    inverse_bind_transform: Mat4,
    inverse_bind_translation: Vec3,
    inverse_bind_scale: Vec3,
    inverse_bind_rotation: Quat,

}

impl Joint {
    pub fn new(name: String, flags: u16, id: i16, parent_id: i16, radius: f32, local_transform: Mat4, inverse_bind_transform: Mat4) -> Self {
        let (local_scale, local_rotation, local_translation) = local_transform.to_scale_rotation_translation();
        let (inverse_bind_scale, inverse_bind_rotation, inverse_bind_translation) = inverse_bind_transform.to_scale_rotation_translation();

        Self {
            name,
            flags,
            id,
            parent_id,
            radius,
            local_transform,
            local_translation,
            local_scale,
            local_rotation,
            inverse_bind_transform,
            inverse_bind_translation,
            inverse_bind_scale,
            inverse_bind_rotation,
        }
    }

    pub fn from_reader<R: Read + Seek + ?Sized>(reader: &mut R) -> io::Result<Self> {
        use byteorder::{ReadBytesExt as _, LE};
        use crate::util::ReaderExt as _;

        let flags = reader.read_u16::<LE>()?;
        let id = reader.read_i16::<LE>()?;
        let parent_id = reader.read_i16::<LE>()?;
        reader.read_i16::<LE>()?; // padding
        let name_hash = reader.read_u32::<LE>()?;
        let radius = reader.read_f32::<LE>()?;

        let local_translation = reader.read_vec3::<LE>()?;
        let local_scale = reader.read_vec3::<LE>()?;
        let local_rotation = reader.read_quat::<LE>()?.normalize();

        let inverse_bind_translation = reader.read_vec3::<LE>()?;
        let inverse_bind_scale = reader.read_vec3::<LE>()?;
        let inverse_bind_rotation = reader.read_quat::<LE>()?;

        let name_off = reader.read_i32::<LE>()?;
        let return_pos = reader.stream_position()?;

        reader.seek(SeekFrom::Current(-4 + name_off as i64))?;
        let name = reader.read_str_until_nul()?;
        reader.seek(SeekFrom::Start(return_pos))?;

        Ok(Self {
            name,
            flags,
            id,
            parent_id,
            radius,
            local_transform: Mat4::from_scale_rotation_translation(local_scale, local_rotation.normalize(), local_translation),
            local_translation,
            local_scale,
            local_rotation,
            inverse_bind_transform: Mat4::from_scale_rotation_translation(inverse_bind_scale, inverse_bind_rotation.normalize(), inverse_bind_translation),
            inverse_bind_translation,
            inverse_bind_scale,
            inverse_bind_rotation,
        })
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn flags(&self) -> u16 {
        self.flags
    }
    pub fn id(&self) -> i16 {
        self.id
    }
    pub fn parent_id(&self) -> i16 {
        self.parent_id
    }
    pub fn radius(&self) -> f32 {
        self.radius
    }
    pub fn local_transform(&self) -> Mat4 {
        self.local_transform
    }
    pub fn local_translation(&self) -> Vec3 {
        self.local_translation
    }
    pub fn local_scale(&self) -> Vec3 {
        self.local_scale
    }
    pub fn local_rotation(&self) -> Quat {
        self.local_rotation
    }
    pub fn inverse_bind_transform(&self) -> Mat4 {
        self.inverse_bind_transform
    }
    pub fn inverse_bind_translation(&self) -> Vec3 {
        self.inverse_bind_translation
    }
    pub fn inverse_bind_scale(&self) -> Vec3 {
        self.inverse_bind_scale
    }
    pub fn inverse_bind_rotation(&self) -> Quat {
        self.inverse_bind_rotation
    }
}


#[derive(Debug, Clone, PartialEq)]
pub struct LegacyJoint {
    name: String,
    id: i16,
    parent_id: i16,
    radius: f32,
    global_transform: Mat4,
}

impl LegacyJoint {
    pub fn from_reader<R: Read + ?Sized>(reader: &mut R, id: i16) -> io::Result<Self> {
        use byteorder::{ReadBytesExt as _, LE};
        use crate::util::ReaderExt as _;

        let name = reader.read_padded_string::<LE, 32>().expect("FIXME: better error here");
        let parent_id = reader.read_i32::<LE>()? as i16;
        let radius = reader.read_f32::<LE>()?;
        let mut transform = [[0.0; 4]; 4];
        transform[3][3] = 1.0;
        for i in 0..3 {
            for j in 0..4 {
                transform[j][i] = reader.read_f32::<LE>()?;
            }
        }

        Ok(Self {
            name,
            id,
            parent_id,
            radius,
            global_transform: Mat4::from_cols_array_2d(&transform),
        })
    }
}