use std::{collections::HashMap, io};

use crate::core::meta::ParseError;

use super::{BinTree, BinTreeObject};
use crate::util::ReaderExt as _;
use byteorder::{ReadBytesExt as _, LE};

impl BinTree {
    pub const PROP: u32 = u32::from_le_bytes(*b"PROP");
    pub const PTCH: u32 = u32::from_le_bytes(*b"PTCH");
    pub fn from_reader<R: io::Read + std::io::Seek + ?Sized>(
        reader: &mut R,
    ) -> Result<Self, ParseError> {
        let magic = reader.read_u32::<LE>()?;
        let is_override = match magic {
            Self::PROP => false,
            Self::PTCH => {
                let override_version = reader.read_u32::<LE>()?;
                if override_version != 1 {
                    return Err(ParseError::InvalidFileVersion(override_version));
                }

                // It might be possible to create an override property bin
                // and set the original file as a dependency.
                // This seems to be the object count of the override section
                let _maybe_override_object_count = reader.read_u32::<LE>()?;

                let magic = reader.read_u32::<LE>()?;
                if magic != Self::PROP {
                    // TODO (alan): repr this in the error
                    log::error!(
                        "Expected PROP ({:#x}) section after PTCH ({:#x}), got '{:#x}'",
                        Self::PROP,
                        Self::PTCH,
                        magic
                    );
                    return Err(ParseError::InvalidFileSignature);
                }
                true
            }
            _ => return Err(ParseError::InvalidFileSignature),
        };

        let version = reader.read_u32::<LE>()?;
        if !matches!(version, 1..=3) {
            // TODO (alan): distinguish override/non-override version
            return Err(ParseError::InvalidFileVersion(version));
        }

        let dependencies = match version {
            2.. => {
                let dep_count = reader.read_u32::<LE>()?;
                let mut dependencies = Vec::with_capacity(dep_count as _);
                for _ in 0..dep_count {
                    dependencies.push(reader.read_len_prefixed_string::<LE>()?);
                }
                dependencies
            }
            _ => Vec::new(),
        };

        let obj_count = reader.read_u32::<LE>()? as usize;
        let mut obj_classes = Vec::with_capacity(obj_count);
        for _ in 0..obj_count {
            obj_classes.push(reader.read_u32::<LE>()?);
        }

        let mut objects = HashMap::with_capacity(obj_count);
        match Self::try_read_objects(reader, &obj_classes, &mut objects, false) {
            Ok(_) => {}
            Err(ParseError::InvalidPropertyTypePrimitive(kind)) => {
                log::warn!("Invalid prop type {kind}. Trying reading objects as legacy.");
                Self::try_read_objects(reader, &obj_classes, &mut objects, true)?;
            }
            e => e?,
        }

        let data_overrides = match (is_override, version) {
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
            version,
            is_override,
            objects,
            dependencies,
            data_overrides,
        })
    }

    fn try_read_objects<R: io::Read + std::io::Seek + ?Sized>(
        reader: &mut R,
        obj_classes: &[u32],
        objects: &mut HashMap<u32, BinTreeObject>,
        legacy: bool,
    ) -> Result<(), ParseError> {
        objects.clear();
        for &class_hash in obj_classes {
            let tree_obj = BinTreeObject::from_reader(reader, class_hash, legacy)?;
            objects.insert(tree_obj.path_hash, tree_obj);
        }
        Ok(())
    }
}