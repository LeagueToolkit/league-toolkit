use std::{borrow::Cow, fmt, str::FromStr};

use indexmap::Equivalent;

use crate::ast::Value;

/// One of the four entries every ritobin file has at its root.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[deprecated]
pub enum RootKind {
    Type,
    Version,
    Linked,
    Entries,
}

impl RootKind {
    /// The name this root entry is written with in a ritobin file.
    pub fn as_str(&self) -> &'static str {
        match self {
            RootKind::Type => "type",
            RootKind::Version => "version",
            RootKind::Linked => "linked",
            RootKind::Entries => "entries",
        }
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
            "type" => RootKind::Type,
            "version" => RootKind::Version,
            "linked" => RootKind::Linked,
            "entries" => RootKind::Entries,
            _ => return Err(()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[deprecated]
pub enum RootKindOrUnknown<'a> {
    Known(RootKind),
    Unknown(Cow<'a, str>),
}

impl std::hash::Hash for RootKindOrUnknown<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            RootKindOrUnknown::Known(root_kind) => root_kind.hash(state),
            RootKindOrUnknown::Unknown(cow) => cow.hash(state),
        }
    }
}

impl Equivalent<RootKindOrUnknown<'_>> for RootKind {
    #[inline(always)]
    fn equivalent(&self, key: &RootKindOrUnknown<'_>) -> bool {
        match key {
            RootKindOrUnknown::Known(root_kind) => self == root_kind,
            RootKindOrUnknown::Unknown(_) => false,
        }
    }
}
impl Equivalent<RootKindOrUnknown<'_>> for Cow<'_, str> {
    #[inline(always)]
    fn equivalent(&self, key: &RootKindOrUnknown<'_>) -> bool {
        match key {
            RootKindOrUnknown::Known(_) => false,
            RootKindOrUnknown::Unknown(cow) => self == cow,
        }
    }
}
impl Equivalent<RootKindOrUnknown<'_>> for str {
    #[inline(always)]
    fn equivalent(&self, key: &RootKindOrUnknown<'_>) -> bool {
        match key {
            RootKindOrUnknown::Known(_) => false,
            RootKindOrUnknown::Unknown(cow) => self == cow,
        }
    }
}

impl<'a> RootKindOrUnknown<'a> {
    pub fn from_value(src: &'a str, value: &Value) -> Self {
        let Value::String(string) = value else {
            return Self::Unknown(src[value.span()].into());
        };

        let value = string.value.as_str();
        match value.parse() {
            Ok(kind) => Self::Known(kind),
            Err(_) => Self::Unknown(src[string.span].into()),
        }
    }
}

impl From<RootKind> for RootKindOrUnknown<'_> {
    #[inline(always)]
    fn from(value: RootKind) -> Self {
        Self::Known(value)
    }
}
impl<'a> From<Cow<'a, str>> for RootKindOrUnknown<'a> {
    #[inline(always)]
    fn from(value: Cow<'a, str>) -> Self {
        Self::Unknown(value)
    }
}

#[cfg(test)]
mod test {
    use indexmap::IndexMap;

    use super::*;

    #[test]
    fn root_kind_eq() {
        let mut root: IndexMap<RootKindOrUnknown<'static>, ()> = Default::default();

        root.insert(RootKind::Version.into(), ());
        root.insert(RootKind::Entries.into(), ());
        root.insert(RootKindOrUnknown::Unknown("foo".into()), ());
        root.insert(RootKindOrUnknown::Unknown("bar".into()), ());

        assert!(root.swap_remove(&RootKind::Version).is_some());
        assert!(root.swap_remove(&RootKind::Entries).is_some());
        assert!(root
            .swap_remove(&RootKindOrUnknown::Unknown("bar".into()))
            .is_some());

        assert_eq!(root.len(), 1);
    }
}
