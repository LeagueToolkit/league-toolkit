use std::io;

use crate::{header::Header, Error};

use super::{Bin, BinObject};
use byteorder::{ReadBytesExt as _, LE};
use indexmap::IndexMap;
use ltk_hash::{BinHash, ReadBytesExt as _};

impl Bin {
    pub const PROP: u32 = u32::from_le_bytes(*b"PROP");
    pub const PTCH: u32 = u32::from_le_bytes(*b"PTCH");

    /// Reads a BinTree from a reader.
    ///
    /// # Arguments
    ///
    /// * `reader` - A reader that implements io::Read and io::Seek.
    pub fn from_reader<R: io::Read + std::io::Seek + ?Sized>(
        reader: &mut R,
    ) -> Result<Self, Error> {
        let (prop, patch) = Header::from_reader(reader)?.into_parts();

        let obj_count = prop.object_count.try_into().unwrap();
        let mut obj_classes = Vec::with_capacity(obj_count);
        for _ in 0..obj_count {
            obj_classes.push(reader.read_bin_hash::<LE>()?);
        }

        let mut objects = IndexMap::with_capacity(obj_count);
        match Self::try_read_objects(reader, &obj_classes, &mut objects, false) {
            Ok(_) => {}
            Err(Error::InvalidPropertyTypePrimitive(kind)) => {
                log::warn!("Invalid prop type {kind}. Trying reading objects as legacy.");
                Self::try_read_objects(reader, &obj_classes, &mut objects, true)?;
            }
            e => e?,
        }

        let data_overrides = match (patch.is_some(), prop.version) {
            (true, 3..) => {
                let count = reader.read_u32::<LE>()?;
                let mut v = Vec::with_capacity(count as _);
                for _ in 0..count {
                    v.push(()); // TODO: impl data overrides
                }
                v
            }
            _ => Vec::new(),
        };

        Ok(Self {
            version: prop.version,
            is_override: patch.is_some(),
            objects,
            dependencies: prop
                .dependencies
                .unwrap_or_default()
                .into_iter()
                .map(|d| d.into())
                .collect(),
            data_overrides,
        })
    }

    fn try_read_objects<R: io::Read + std::io::Seek + ?Sized>(
        reader: &mut R,
        obj_classes: &[BinHash],
        objects: &mut IndexMap<BinHash, BinObject>,
        legacy: bool,
    ) -> Result<(), Error> {
        objects.clear();
        for &class_hash in obj_classes {
            let tree_obj = BinObject::from_reader(reader, class_hash, legacy)?;
            objects.insert(tree_obj.path_hash, tree_obj);
        }
        Ok(())
    }
}
