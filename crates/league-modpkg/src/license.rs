use binrw::{binrw, NullString};

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Default)]
pub enum ModpkgLicense {
    #[default]
    #[brw(magic = 0u8)]
    None,
    #[brw(magic = 1u8)]
    Spdx { spdx_id: NullString },
    #[brw(magic = 2u8)]
    Custom { name: NullString, url: NullString },
}

impl ModpkgLicense {
    #[inline]
    pub fn size(&self) -> usize {
        1 + match self {
            Self::None => 0,
            // null terminators not included in len()
            Self::Spdx { spdx_id } => 1 + spdx_id.len(),
            Self::Custom { name, url } => 2 + name.len() + url.len(),
        }
    }
}

// TODO: use proptest here
#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use binrw::BinWrite;

    use super::*;

    #[test]
    fn test_none_size() {
        let license = ModpkgLicense::default();
        let mut buf = Cursor::new(Vec::with_capacity(license.size() + 512));
        license.write(&mut buf).unwrap();
        println!("{:x?}", buf.clone().into_inner());

        assert_eq!(
            license.size(),
            buf.into_inner().len(),
            "comparing reported size with real size"
        );
    }

    #[test]
    fn test_spdx_size() {
        let license = ModpkgLicense::Spdx {
            spdx_id: "test".to_string().into(),
        };

        let mut buf = Cursor::new(Vec::with_capacity(license.size() + 512));
        license.write(&mut buf).unwrap();

        assert_eq!(
            license.size(),
            buf.into_inner().len(),
            "comparing reported size with real size"
        );
    }
    #[test]
    fn test_custom_size() {
        let license = ModpkgLicense::Custom {
            name: "customName".into(),
            url: "http://fake.url/".into(),
        };
        let mut buf = Cursor::new(Vec::with_capacity(license.size() + 512));
        license.write(&mut buf).unwrap();

        println!("{:x?}", buf.clone().into_inner());

        assert_eq!(
            license.size(),
            buf.into_inner().len(),
            "comparing reported size with real size"
        );
    }
}
