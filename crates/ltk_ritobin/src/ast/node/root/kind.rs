use std::{fmt, str::FromStr};

use crate::{ast::Value, rito, RitoType};

/// One of the four entries every ritobin file has at its root, or [`Self::Unknown`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RootKind {
    #[default]
    Unknown,
    Version,
    Type,
    Linked,
    Entries,
}

impl RootKind {
    /// The key this root entry uses in a ritobin file.
    /// [`Self::Unknown`] is not an actual root entry,
    /// but is written as `"unknown"` in this method.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Type => "type",
            Self::Version => "version",
            Self::Linked => "linked",
            Self::Entries => "entries",
            Self::Unknown => "unknown",
        }
    }

    /// What type this kind of root expects
    pub fn expected_type(&self) -> Option<RitoType> {
        Some(match self {
            RootKind::Unknown => return None,
            RootKind::Version => rito!(U32),
            RootKind::Type => rito!(String),
            RootKind::Linked => rito!(Container[String]),
            RootKind::Entries => rito!(Map[Hash, Embedded]),
        })
    }

    pub fn from_value(value: &Value) -> Self {
        let Value::String(string) = value else {
            return Self::Unknown;
        };

        let value = string.value.as_str();
        value.parse().unwrap_or(Self::Unknown)
    }
}

impl fmt::Display for RootKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RootKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, ()> {
        Ok(match s {
            "type" => Self::Type,
            "version" => Self::Version,
            "linked" => Self::Linked,
            "entries" => Self::Entries,
            _ => return Err(()),
        })
    }
}
