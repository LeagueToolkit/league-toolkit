//! Bank and package paths, classified by the rules of the client.

use crate::BankId;

/// A path of a `BankUnit`, classified by the rules of the client.
///
/// Every test compares ASCII case-insensitively. Shipped bins author paths in mixed case.
///
/// ```
/// use ltk_wwise::path::{AudioFileKind, AudioPath};
///
/// let path = AudioPath::new("ASSETS/Sounds/Wwise2016/SFX/Characters/Ahri/Skins/Base/Ahri_Base_SFX_events.bnk");
/// assert_eq!(path.kind(), AudioFileKind::Bank);
/// assert!(path.registers_events());
/// assert!(!path.is_localized());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioPath<'a> {
    path: &'a str,
}

impl<'a> AudioPath<'a> {
    /// Wraps the `BankUnit` path `path`.
    pub fn new(path: &'a str) -> Self {
        Self { path }
    }

    /// Returns the kind of file the client loads from the path.
    ///
    /// A path that ends with `.wpk` is a [`AudioFileKind::Package`]. Every other path is a
    /// [`AudioFileKind::Bank`].
    pub fn kind(&self) -> AudioFileKind {
        let bytes = self.path.as_bytes();
        let is_package = bytes
            .len()
            .checked_sub(PACKAGE_EXTENSION.len())
            .is_some_and(|start| bytes[start..].eq_ignore_ascii_case(PACKAGE_EXTENSION));
        if is_package {
            AudioFileKind::Package
        } else {
            AudioFileKind::Bank
        }
    }

    /// Returns `true` when the client collects the event IDs of the bank.
    ///
    /// The client collects them from a path that contains `_events`.
    pub fn registers_events(&self) -> bool {
        contains_ignore_ascii_case(self.path.as_bytes(), b"_events")
    }

    /// Returns `true` when the client searches the localized path table first.
    ///
    /// The client searches it first for a path that contains `en_us`.
    pub fn is_localized(&self) -> bool {
        contains_ignore_ascii_case(self.path.as_bytes(), b"en_us")
    }

    /// Returns the bank ID of the file name of the path.
    ///
    /// The file name is the text after the last `/` or `\`: [`BankId::from_name`] of it.
    pub fn bank_id(&self) -> BankId {
        let file_name = self.path.rsplit(['/', '\\']).next().unwrap_or(self.path);
        BankId::from_name(file_name)
    }
}

/// The kind of file that a `BankUnit` path names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioFileKind {
    /// A bank: the client loads it with `LoadBank`.
    Bank,
    /// A package: the client adds its entries to the path table.
    Package,
}

/// The extension of a package path.
const PACKAGE_EXTENSION: &[u8] = b".wpk";

fn contains_ignore_ascii_case(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}
