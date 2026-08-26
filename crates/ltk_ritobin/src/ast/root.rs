use std::borrow::Cow;

use indexmap::Equivalent;
use ltk_meta::{traits::PropertyExt as _, PropertyValueEnum};

use crate::{
    ast::{diagnostics::RootKind, AstValue},
    parse::Span,
};

#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub fn from_value(src: &'a str, value: &AstValue) -> Self {
        let AstValue::String(string) = value else {
            return Self::Unknown(src[value.span()].into());
        };

        match string.value.as_str() {
            "type" => RootKind::Type.into(),
            "version" => RootKind::Version.into(),
            "linked" => RootKind::Linked.into(),
            "entries" => RootKind::Entries.into(),
            _ => Self::Unknown(src[value.span()].into()),
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
